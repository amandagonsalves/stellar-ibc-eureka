#!/bin/sh
set -e

CONFIG_JSON="${OSMOSIS_CONFIG_JSON:-/config/default-config.json}"
OSMOSIS_HOME=$HOME/.osmosisd
CONFIG_FOLDER=$OSMOSIS_HOME/config
KEYRING="--keyring-backend test --home $OSMOSIS_HOME"
TAB="$(printf '\t')"

apply_overrides () {
    target_file="$1"
    selector="$2"
    jq -r "${selector}[] | [.path, .type, (.value | tostring)] | @tsv" "$CONFIG_JSON" |
    while IFS="$TAB" read -r path type value; do
        dasel put -t "$type" -f "$target_file" -v "$value" "$path"
    done
}

add_keys_and_accounts () {
    jq -r '.accounts | keys[]' "$CONFIG_JSON" |
    while read -r name; do
        mnemonic="$(jq -r --arg n "$name" '.keys[$n]' "$CONFIG_JSON")"
        coins="$(jq -r --arg n "$name" '.accounts[$n]' "$CONFIG_JSON")"
        echo "$mnemonic" | osmosisd keys add "$name" --recover $KEYRING
        address="$(osmosisd keys show "$name" -a $KEYRING)"
        osmosisd add-genesis-account "$address" "$coins" --home "$OSMOSIS_HOME"
    done
}

init_chain () {
    apk add --no-cache jq dasel

    CHAIN_ID="${OSMOSIS_CHAIN_ID:-$(jq -r '.chain_id' "$CONFIG_JSON")}"
    MONIKER="$(jq -r '.moniker' "$CONFIG_JSON")"
    GENESIS_TIME="${OSMOSIS_LOCAL_GENESIS_TIME:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"
    GENTX_KEY="$(jq -r '.gentx.key' "$CONFIG_JSON")"
    GENTX_AMOUNT="$(jq -r '.gentx.amount' "$CONFIG_JSON")"

    osmosisd init -o --chain-id="$CHAIN_ID" --home "$OSMOSIS_HOME" "$MONIKER"

    apply_overrides "$CONFIG_FOLDER/genesis.json" '.genesis'
    dasel put -t string -f "$CONFIG_FOLDER/genesis.json" -v "$GENESIS_TIME" '.genesis_time'

    add_keys_and_accounts
    osmosisd gentx "$GENTX_KEY" "$GENTX_AMOUNT" --chain-id="$CHAIN_ID" $KEYRING
    osmosisd collect-gentxs --home "$OSMOSIS_HOME"

    apply_overrides "$CONFIG_FOLDER/app.toml" '.app'
    apply_overrides "$CONFIG_FOLDER/config.toml" '.config'
}

if [ ! -d "$CONFIG_FOLDER" ]; then
    init_chain
fi

exec osmosisd start --home "$OSMOSIS_HOME"
