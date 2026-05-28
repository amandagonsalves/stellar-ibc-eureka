#!/bin/sh
set -e

CHAIN_ID="${OSMOSIS_CHAIN_ID:-localosmosis}"
OSMOSIS_HOME=$HOME/.osmosisd
CONFIG_FOLDER=$OSMOSIS_HOME/config
MONIKER=val
LOCAL_GENESIS_TIME="${OSMOSIS_LOCAL_GENESIS_TIME:-2025-12-31T23:59:00Z}"

MNEMONIC="bottom loan skill merry east cradle onion journey palm apology verb edit desert impose absurd oil bubble sweet glove shallow size build burst effort"
POOLSMNEMONIC="traffic cool olive pottery elegant innocent aisle dial genuine install shy uncle ride federal soon shift flight program cave famous provide cute pole struggle"

install_prerequisites () {
    apk add --no-cache dasel
}

edit_app_toml () {
    APP=$CONFIG_FOLDER/app.toml
    dasel put -t bool -f $APP '.api.enable' -v true
    dasel put -f $APP '.api.address' -v 'tcp://0.0.0.0:1317'
    dasel put -f $APP '.grpc.address' -v '0.0.0.0:9090'
    dasel put -f $APP '.grpc-web.address' -v '0.0.0.0:9091'
}

edit_genesis () {
    GENESIS=$CONFIG_FOLDER/genesis.json

    dasel put -t string -f $GENESIS '.genesis_time' -v "$LOCAL_GENESIS_TIME"

    dasel put -t string -f $GENESIS '.app_state.staking.params.bond_denom' -v 'uosmo'

    dasel put -t string -f $GENESIS '.app_state.bank.denom_metadata.[].description' -v 'Registered denom uion for localosmosis testing'
    dasel put -t string -f $GENESIS '.app_state.bank.denom_metadata.[0].denom_units.[].denom' -v 'uion'
    dasel put -t string -f $GENESIS '.app_state.bank.denom_metadata.[0].denom_units.[0].exponent' -v 0
    dasel put -t string -f $GENESIS '.app_state.bank.denom_metadata.[0].base' -v 'uion'
    dasel put -t string -f $GENESIS '.app_state.bank.denom_metadata.[0].display' -v 'uion'
    dasel put -t string -f $GENESIS '.app_state.bank.denom_metadata.[0].name' -v 'uion'
    dasel put -t string -f $GENESIS '.app_state.bank.denom_metadata.[0].symbol' -v 'uion'

    dasel put -t string -f $GENESIS '.app_state.bank.denom_metadata.[].description' -v 'Registered denom uosmo for localosmosis testing'
    dasel put -t string -f $GENESIS '.app_state.bank.denom_metadata.[1].denom_units.[].denom' -v 'uosmo'
    dasel put -t string -f $GENESIS '.app_state.bank.denom_metadata.[1].denom_units.[0].exponent' -v 0
    dasel put -t string -f $GENESIS '.app_state.bank.denom_metadata.[1].base' -v 'uosmo'
    dasel put -t string -f $GENESIS '.app_state.bank.denom_metadata.[1].display' -v 'uosmo'
    dasel put -t string -f $GENESIS '.app_state.bank.denom_metadata.[1].name' -v 'uosmo'
    dasel put -t string -f $GENESIS '.app_state.bank.denom_metadata.[1].symbol' -v 'uosmo'

    dasel put -t string -f $GENESIS '.app_state.crisis.constant_fee.denom' -v 'uosmo'
    dasel put -t string -f $GENESIS '.app_state.gov.params.min_deposit.[0].denom' -v 'uosmo'
    dasel put -t string -f $GENESIS '.app_state.poolincentives.params.minted_denom' -v 'uosmo'
    dasel put -t string -f $GENESIS '.app_state.incentives.params.distr_epoch_identifier' -v 'hour'
    dasel put -t string -f $GENESIS '.app_state.mint.params.mint_denom' -v 'uosmo'
    dasel put -t string -f $GENESIS '.app_state.mint.params.epoch_identifier' -v 'hour'
    dasel put -t string -f $GENESIS '.app_state.poolmanager.params.pool_creation_fee.[0].denom' -v 'uosmo'
    dasel put -t string -f $GENESIS '.app_state.txfees.basedenom' -v 'uosmo'
    dasel put -t string -f $GENESIS '.app_state.wasm.params.code_upload_access.permission' -v 'Everybody'
    dasel put -t bool -f $GENESIS '.app_state.concentratedliquidity.params.is_permissionless_pool_creation_enabled' -v true
}

add_genesis_accounts () {
    osmosisd add-genesis-account osmo12smx2wdlyttvyzvzg54y2vnqwq2qjateuf7thj 100000000000uosmo,100000000000uion,100000000000stake,100000000000uusdc,100000000000uweth --home $OSMOSIS_HOME
    osmosisd add-genesis-account osmo1cyyzpxplxdzkeea7kwsydadg87357qnahakaks 9999999999999999999999999999999999999999999999999uosmo,9999999999999999999999999999999999999999999999999uion,100000000000stake,100000000000uusdc,100000000000uweth --home $OSMOSIS_HOME
    osmosisd add-genesis-account osmo18s5lynnmx37hq4wlrw9gdn68sg2uxp5rgk26vv 100000000000uosmo,100000000000uion,100000000000stake,100000000000uusdc,100000000000uweth --home $OSMOSIS_HOME
    osmosisd add-genesis-account osmo1qwexv7c6sm95lwhzn9027vyu2ccneaqad4w8ka 100000000000uosmo,100000000000uion,100000000000stake,100000000000uusdc,100000000000uweth --home $OSMOSIS_HOME
    osmosisd add-genesis-account osmo14hcxlnwlqtq75ttaxf674vk6mafspg8xwgnn53 100000000000uosmo,100000000000uion,100000000000stake,100000000000uusdc,100000000000uweth --home $OSMOSIS_HOME
    osmosisd add-genesis-account osmo12rr534cer5c0vj53eq4y32lcwguyy7nndt0u2t 100000000000uosmo,100000000000uion,100000000000stake,100000000000uusdc,100000000000uweth --home $OSMOSIS_HOME
    osmosisd add-genesis-account osmo1nt33cjd5auzh36syym6azgc8tve0jlvklnq7jq 100000000000uosmo,100000000000uion,100000000000stake,100000000000uusdc,100000000000uweth --home $OSMOSIS_HOME
    osmosisd add-genesis-account osmo10qfrpash5g2vk3hppvu45x0g860czur8ff5yx0 100000000000uosmo,100000000000uion,100000000000stake,100000000000uusdc,100000000000uweth --home $OSMOSIS_HOME
    osmosisd add-genesis-account osmo1f4tvsdukfwh6s9swrc24gkuz23tp8pd3e9r5fa 100000000000uosmo,100000000000uion,100000000000stake,100000000000uusdc,100000000000uweth --home $OSMOSIS_HOME
    osmosisd add-genesis-account osmo1myv43sqgnj5sm4zl98ftl45af9cfzk7nhjxjqh 100000000000uosmo,100000000000uion,100000000000stake,100000000000uusdc,100000000000uweth --home $OSMOSIS_HOME
    osmosisd add-genesis-account osmo14gs9zqh8m49yy9kscjqu9h72exyf295afg6kgk 100000000000uosmo,100000000000uion,100000000000stake,100000000000uusdc,100000000000uweth --home $OSMOSIS_HOME
    osmosisd add-genesis-account osmo1jllfytsz4dryxhz5tl7u73v29exsf80vz52ucc 1000000000000uosmo,1000000000000uion,1000000000000stake,1000000000000uusdc,1000000000000uweth --home $OSMOSIS_HOME

    echo $MNEMONIC | osmosisd keys add $MONIKER --recover --keyring-backend=test --home $OSMOSIS_HOME
    echo $POOLSMNEMONIC | osmosisd keys add pools --recover --keyring-backend=test --home $OSMOSIS_HOME
    osmosisd gentx $MONIKER 500000000uosmo --keyring-backend=test --chain-id=$CHAIN_ID --home $OSMOSIS_HOME
    osmosisd collect-gentxs --home $OSMOSIS_HOME
}

edit_config () {
    dasel put -t string -f $CONFIG_FOLDER/config.toml '.p2p.seeds' -v ''
    dasel put -t string -f $CONFIG_FOLDER/config.toml '.rpc.laddr' -v 'tcp://0.0.0.0:26657'
}

enable_cors () {
    dasel put -t string -f $CONFIG_FOLDER/config.toml -v '*' '.rpc.cors_allowed_origins.[]'
    dasel put -t string -f $CONFIG_FOLDER/config.toml -v 'Accept-Encoding' '.rpc.cors_allowed_headers.[]'
    dasel put -t string -f $CONFIG_FOLDER/config.toml -v 'DELETE' '.rpc.cors_allowed_methods.[]'
    dasel put -t string -f $CONFIG_FOLDER/config.toml -v 'OPTIONS' '.rpc.cors_allowed_methods.[]'
    dasel put -t string -f $CONFIG_FOLDER/config.toml -v 'PATCH' '.rpc.cors_allowed_methods.[]'
    dasel put -t string -f $CONFIG_FOLDER/config.toml -v 'PUT' '.rpc.cors_allowed_methods.[]'
    dasel put -t bool -f $CONFIG_FOLDER/app.toml -v true '.api.swagger'
    dasel put -t bool -f $CONFIG_FOLDER/app.toml -v true '.api.enabled-unsafe-cors'
    dasel put -t bool -f $CONFIG_FOLDER/app.toml -v true '.grpc-web.enable-unsafe-cors'
}

if [ ! -d "$CONFIG_FOLDER" ]; then
    echo $MNEMONIC | osmosisd init -o --chain-id=$CHAIN_ID --home $OSMOSIS_HOME --recover $MONIKER
    install_prerequisites
    edit_genesis
    add_genesis_accounts
    edit_app_toml
    edit_config
    enable_cors
fi

exec osmosisd start --home $OSMOSIS_HOME
