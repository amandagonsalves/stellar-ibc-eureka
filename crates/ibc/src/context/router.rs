use ibc::core::{
    host::types::identifiers::PortId,
    router::{module::Module, router::ModuleId, types::Router},
};

pub struct IbcRouter;

impl Router for IbcRouter {
    fn get_route(&self, _module_id: &ModuleId) -> Option<&dyn Module> {
        None
    }

    fn get_route_mut(&mut self, _module_id: &ModuleId) -> Option<&mut dyn Module> {
        None
    }

    fn lookup_module(&self, _port_id: &PortId) -> Option<ModuleId> {
        None
    }
}
