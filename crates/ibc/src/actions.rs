use crate::{
    context::{router::IbcRouter, storage::SorobanStorage, StellarIbcContext},
    error::Error,
    event::IbcEvent,
    msg::decode_message,
};

pub struct IbcActions<S: SorobanStorage> {
    ctx: StellarIbcContext<S>,
    router: IbcRouter,
}

impl<S: SorobanStorage> IbcActions<S> {
    pub fn new(store: S) -> Self {
        Self {
            ctx: StellarIbcContext::new(store),
            router: IbcRouter,
        }
    }

    pub fn execute(&mut self, tx_data: &[u8]) -> Result<Vec<IbcEvent>, Error> {
        let ibc_msg = decode_message(tx_data)?;
        let crate::msg::IbcMessage::Envelope(envelope) = ibc_msg;

        ibc::core::entrypoint::dispatch(&mut self.ctx, &mut self.router, *envelope)
            .map_err(|e| Error::Handler(Box::new(e)))?;

        Ok(std::mem::take(&mut self.ctx.events))
    }
}
