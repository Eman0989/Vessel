use crate::{
    RuntimeError, RuntimeLimits,
    bindings::VesselWorkload,
    limits::{EPOCH_TICK_INTERVAL, StoreState},
};
use vessel_policy::{CapabilityPolicy, DirectoryAccess, NetworkAccess};
use wasmtime::component::{Component, Linker as ComponentLinker};
use wasmtime::{Config, Engine, Instance, Module, Store, Trap};
use wasmtime_wasi::filesystem::WasiFilesystemView;
use wasmtime_wasi::sockets::WasiSocketsView;
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder};

#[derive(Clone)]
pub struct WasmRuntime {
    engine: Engine,
    limits: RuntimeLimits,
    policy: CapabilityPolicy,
}

impl WasmRuntime {
    pub fn new() -> Self {
        Self::with_limits(RuntimeLimits::default())
            .expect("default VESSEL Wasmtime configuration must be valid")
    }

    pub fn with_limits(limits: RuntimeLimits) -> Result<Self, RuntimeError> {
        Self::with_limits_and_policy(limits, CapabilityPolicy::deny_all())
    }

    pub fn with_limits_and_policy(
        limits: RuntimeLimits,
        policy: CapabilityPolicy,
    ) -> Result<Self, RuntimeError> {
        policy.validate()?;

        let mut config = Config::new();

        config.consume_fuel(true);
        config.epoch_interruption(true);

        let engine = Engine::new(&config).map_err(RuntimeError::Initialize)?;

        Self::start_epoch_ticker(&engine);

        Ok(Self {
            engine,
            limits,
            policy,
        })
    }

    fn start_epoch_ticker(engine: &Engine) {
        let weak_engine = engine.weak();

        let _ticker = std::thread::spawn(move || {
            loop {
                std::thread::sleep(EPOCH_TICK_INTERVAL);

                let Some(engine) = weak_engine.upgrade() else {
                    break;
                };

                engine.increment_epoch();
            }
        });
    }

    fn epoch_deadline_ticks(&self) -> u64 {
        let timeout_nanos = self.limits.timeout.as_nanos();
        let tick_nanos = EPOCH_TICK_INTERVAL.as_nanos();

        let ticks = timeout_nanos.div_ceil(tick_nanos);

        u64::try_from(ticks).unwrap_or(u64::MAX).max(1)
    }

    fn timeout_ms(&self) -> u64 {
        u64::try_from(self.limits.timeout.as_millis()).unwrap_or(u64::MAX)
    }

    fn build_wasi_ctx(&self) -> Result<WasiCtx, RuntimeError> {
        let mut builder = WasiCtxBuilder::new();

        for (name, value) in &self.policy.environment {
            builder.env(name, value);
        }

        for directory in &self.policy.directories {
            let (dir_perms, file_perms) = match directory.access {
                DirectoryAccess::ReadOnly => (DirPerms::READ, FilePerms::READ),

                DirectoryAccess::ReadWrite => (
                    DirPerms::READ | DirPerms::MUTATE,
                    FilePerms::READ | FilePerms::WRITE,
                ),
            };

            builder
                .preopened_dir(
                    &directory.host_path,
                    &directory.guest_path,
                    dir_perms,
                    file_perms,
                )
                .map_err(RuntimeError::WasiContext)?;
        }

        match self.policy.network {
            NetworkAccess::DenyAll => {
                builder.allow_tcp(false);
                builder.allow_udp(false);
                builder.allow_ip_name_lookup(false);
            }

            NetworkAccess::AllowAll => {
                builder.inherit_network();
                builder.allow_ip_name_lookup(true);
            }
        }

        Ok(builder.build())
    }

    fn component_linker(&self) -> Result<ComponentLinker<StoreState>, RuntimeError> {
        let mut linker = ComponentLinker::<StoreState>::new(&self.engine);

        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(RuntimeError::WasiLinker)?;

        Ok(linker)
    }

    fn new_store(&self) -> Result<Store<StoreState>, RuntimeError> {
        let wasi = self.build_wasi_ctx()?;
        let state = StoreState::new(self.limits, wasi);

        let mut store = Store::new(&self.engine, state);

        store.limiter(|state| &mut state.limits);

        store
            .set_fuel(self.limits.fuel)
            .map_err(RuntimeError::Budget)?;

        store.epoch_deadline_trap();
        store.set_epoch_deadline(self.epoch_deadline_ticks());

        Ok(store)
    }

    fn map_execute_error(&self, error: wasmtime::Error) -> RuntimeError {
        if error.downcast_ref::<Trap>() == Some(&Trap::Interrupt) {
            return RuntimeError::Timeout {
                timeout_ms: self.timeout_ms(),
                source: error,
            };
        }

        RuntimeError::Execute(error)
    }

    fn map_component_execute_error(&self, error: wasmtime::Error) -> RuntimeError {
        if error.downcast_ref::<Trap>() == Some(&Trap::Interrupt) {
            return RuntimeError::Timeout {
                timeout_ms: self.timeout_ms(),
                source: error,
            };
        }

        RuntimeError::ComponentExecute(error)
    }

    pub fn invoke_i32_binary(
        &self,
        module_bytes: &[u8],
        export: &str,
        lhs: i32,
        rhs: i32,
    ) -> Result<i32, RuntimeError> {
        let module = Module::new(&self.engine, module_bytes).map_err(RuntimeError::Compile)?;

        let mut store = self.new_store()?;

        let instance =
            Instance::new(&mut store, &module, &[]).map_err(RuntimeError::Instantiate)?;

        let function = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, export)
            .map_err(|source| RuntimeError::Export {
                export: export.to_string(),
                source,
            })?;

        function
            .call(&mut store, (lhs, rhs))
            .map_err(|error| self.map_execute_error(error))
    }

    pub fn invoke_component_i32_binary(
        &self,
        component_bytes: &[u8],
        export: &str,
        lhs: i32,
        rhs: i32,
    ) -> Result<i32, RuntimeError> {
        let component = Component::new(&self.engine, component_bytes)
            .map_err(RuntimeError::ComponentCompile)?;

        let linker = self.component_linker()?;

        let mut store = self.new_store()?;

        let instance = linker
            .instantiate(&mut store, &component)
            .map_err(RuntimeError::ComponentInstantiate)?;

        let function = instance
            .get_typed_func::<(i32, i32), (i32,)>(&mut store, export)
            .map_err(|source| RuntimeError::ComponentExport {
                export: export.to_string(),
                source,
            })?;

        let (result,) = function
            .call(&mut store, (lhs, rhs))
            .map_err(|error| self.map_component_execute_error(error))?;

        Ok(result)
    }

    pub fn invoke_wit_bound_add(
        &self,
        component_bytes: &[u8],
        lhs: i32,
        rhs: i32,
    ) -> Result<i32, RuntimeError> {
        let component = Component::new(&self.engine, component_bytes)
            .map_err(RuntimeError::ComponentCompile)?;

        let linker = self.component_linker()?;

        let mut store = self.new_store()?;

        let bindings = VesselWorkload::instantiate(&mut store, &component, &linker)
            .map_err(RuntimeError::ComponentInstantiate)?;

        bindings
            .call_add(&mut store, lhs, rhs)
            .map_err(|error| self.map_component_execute_error(error))
    }

    pub fn invoke_wasi_environment(
        &self,
        component_bytes: &[u8],
    ) -> Result<Vec<(String, String)>, RuntimeError> {
        let component = Component::new(&self.engine, component_bytes)
            .map_err(RuntimeError::ComponentCompile)?;

        let linker = self.component_linker()?;
        let mut store = self.new_store()?;

        linker
            .instantiate(&mut store, &component)
            .map_err(RuntimeError::ComponentInstantiate)?;

        let mut cli = wasmtime_wasi::cli::WasiCliView::cli(store.data_mut());

        wasmtime_wasi::p2::bindings::cli::environment::Host::get_environment(&mut cli)
            .map_err(RuntimeError::WasiContext)
    }

    pub fn wasi_preopened_directories(&self) -> Result<Vec<String>, RuntimeError> {
        let mut store = self.new_store()?;

        let mut filesystem = store.data_mut().filesystem();

        let directories = wasmtime_wasi::p2::bindings::filesystem::preopens::Host::get_directories(
            &mut filesystem,
        )
        .map_err(RuntimeError::WasiContext)?;

        Ok(directories
            .into_iter()
            .map(|(_, guest_path)| guest_path)
            .collect())
    }

    pub fn wasi_can_create_file(&self, file_name: &str) -> Result<bool, RuntimeError> {
        use wasmtime_wasi::p2::bindings::sync::filesystem::{preopens, types};

        let mut store = self.new_store()?;
        let mut filesystem = store.data_mut().filesystem();

        let directories =
            preopens::Host::get_directories(&mut filesystem).map_err(RuntimeError::WasiContext)?;

        let Some((directory, _)) = directories.into_iter().next() else {
            return Ok(false);
        };

        let result = types::HostDescriptor::open_at(
            &mut filesystem,
            directory,
            types::PathFlags::empty(),
            file_name.to_string(),
            types::OpenFlags::CREATE,
            types::DescriptorFlags::WRITE,
        );

        match result {
            Ok(_) => Ok(true),

            Err(error)
                if matches!(
                    error.downcast_ref(),
                    Some(
                    wasmtime_wasi::p2::bindings::filesystem::types::ErrorCode::ReadOnly
                        | wasmtime_wasi::p2::bindings::filesystem::types::ErrorCode::NotPermitted
                )
                ) =>
            {
                Ok(false)
            }

            Err(error) => Err(RuntimeError::WasiContext(wasmtime::Error::msg(
                error.to_string(),
            ))),
        }
    }

    pub fn wasi_can_resolve_name(&self, name: &str) -> Result<bool, RuntimeError> {
        use wasmtime_wasi::p2::bindings::sockets::{instance_network, ip_name_lookup, network};

        let mut store = self.new_store()?;
        let mut sockets = store.data_mut().sockets();

        let network_handle = instance_network::Host::instance_network(&mut sockets)
            .map_err(RuntimeError::WasiContext)?;

        let result =
            ip_name_lookup::Host::resolve_addresses(&mut sockets, network_handle, name.to_string());

        match result {
            Ok(_) => Ok(true),

            Err(error)
                if matches!(
                    error.downcast_ref(),
                    Some(network::ErrorCode::PermanentResolverFailure)
                        | Some(network::ErrorCode::AccessDenied)
                ) =>
            {
                Ok(false)
            }

            Err(error) => Err(RuntimeError::WasiContext(wasmtime::Error::msg(
                error.to_string(),
            ))),
        }
    }

    pub fn wasi_can_connect_tcp_ipv4(&self, port: u16) -> Result<bool, RuntimeError> {
        use wasmtime::component::Resource;
        use wasmtime_wasi::p2::bindings::{
            sockets::{instance_network, network, tcp_create_socket},
            sync::{io::poll, sockets::tcp},
        };

        let mut store = self.new_store()?;
        let mut sockets = store.data_mut().sockets();

        let network_handle = instance_network::Host::instance_network(&mut sockets)
            .map_err(RuntimeError::WasiContext)?;

        let network_rep = network_handle.rep();

        let socket = match tcp_create_socket::Host::create_tcp_socket(
            &mut sockets,
            network::IpAddressFamily::Ipv4,
        ) {
            Ok(socket) => socket,

            Err(error)
                if matches!(error.downcast_ref(), Some(network::ErrorCode::AccessDenied)) =>
            {
                return Ok(false);
            }

            Err(error) => {
                return Err(RuntimeError::WasiContext(wasmtime::Error::msg(
                    error.to_string(),
                )));
            }
        };

        let socket_rep = socket.rep();

        let address = network::IpSocketAddress::Ipv4(network::Ipv4SocketAddress {
            port,
            address: (127, 0, 0, 1),
        });

        let start_result = tcp::HostTcpSocket::start_connect(
            &mut sockets,
            Resource::new_borrow(socket_rep),
            Resource::new_borrow(network_rep),
            address,
        );

        match start_result {
            Ok(()) => {}

            Err(error) if matches!(error.downcast_ref(), Some(tcp::ErrorCode::AccessDenied)) => {
                return Ok(false);
            }

            Err(error) => {
                return Err(RuntimeError::WasiContext(wasmtime::Error::msg(
                    error.to_string(),
                )));
            }
        }

        let pollable =
            tcp::HostTcpSocket::subscribe(&mut sockets, Resource::new_borrow(socket_rep))
                .map_err(RuntimeError::WasiContext)?;

        poll::Host::poll(&mut store.data_mut().table, vec![pollable])
            .map_err(RuntimeError::WasiContext)?;

        let mut sockets = store.data_mut().sockets();

        match tcp::HostTcpSocket::finish_connect(&mut sockets, Resource::new_borrow(socket_rep)) {
            Ok(_) => Ok(true),

            Err(error) if matches!(error.downcast_ref(), Some(tcp::ErrorCode::AccessDenied)) => {
                Ok(false)
            }

            Err(error) => Err(RuntimeError::WasiContext(wasmtime::Error::msg(
                error.to_string(),
            ))),
        }
    }

    pub fn policy(&self) -> &CapabilityPolicy {
        &self.policy
    }

    pub fn limits(&self) -> RuntimeLimits {
        self.limits
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new()
    }
}
