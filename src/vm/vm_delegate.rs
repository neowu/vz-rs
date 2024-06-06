use std::process;
use std::sync::Arc;
use std::time::Duration;

use block2::StackBlock;
use dispatch::Queue;
use objc2::declare_class;
use objc2::msg_send_id;
use objc2::mutability;
use objc2::rc::Retained;
use objc2::ClassType;
use objc2::DeclaredClass;
use objc2_foundation::run_on_main;
use objc2_foundation::MainThreadBound;
use objc2_foundation::NSError;
use objc2_foundation::NSObject;
use objc2_foundation::NSObjectProtocol;
use objc2_virtualization::VZNetworkDevice;
use objc2_virtualization::VZVirtualMachine;
use objc2_virtualization::VZVirtualMachineDelegate;
use tracing::error;
use tracing::info;

declare_class!(
    pub struct VMDelegate;

    unsafe impl ClassType for VMDelegate {
        type Super = NSObject;
        type Mutability = mutability::Immutable;
        const NAME: &'static str = "VMDelegate";
    }

    impl DeclaredClass for VMDelegate {
    }

    unsafe impl NSObjectProtocol for VMDelegate {}

    unsafe impl VZVirtualMachineDelegate for VMDelegate {
        #[method(guestDidStopVirtualMachine:)]
        fn guest_did_stop_virtual_machine(&self, _: &VZVirtualMachine) {
            info!("guest has stopped the vm");
            process::exit(0);
        }

        #[method(virtualMachine:didStopWithError:)]
        fn virtual_machine_did_stop_with_error(&self, _: &VZVirtualMachine, err: &NSError) {
            error!("guest has stopped the vm due to error, error={}", err.localizedDescription());
            process::exit(1);
        }

        #[method(virtualMachine:networkDevice:attachmentWasDisconnectedWithError:)]
        fn virtual_machine_network_device_attachment_was_disconnected_with_error(&self, _: &VZVirtualMachine, network_device: &VZNetworkDevice, err: &NSError) {
            error!("vm network disconnected, device={network_device:?}, error={}", err.localizedDescription());
            process::exit(1);
        }
    }
);

impl VMDelegate {
    pub fn new() -> Retained<Self> {
        unsafe { msg_send_id![Self::alloc(), init] }
    }
}

pub fn start_vm(vm: Arc<MainThreadBound<Retained<VZVirtualMachine>>>) {
    run_on_main(|marker| {
        info!("start vm");
        let vm = vm.get(marker);
        let block = &StackBlock::new(|err: *mut NSError| {
            if err.is_null() {
                info!("vm started");
            } else {
                error!("vm failed to start, error={}", unsafe { (*err).localizedDescription() });
                process::exit(1);
            }
        });
        unsafe {
            vm.startWithCompletionHandler(block);
        }
    });
}

pub fn stop_vm(vm: Arc<MainThreadBound<Retained<VZVirtualMachine>>>) {
    run_on_main(|marker| {
        info!("stop vm");
        if request_stop_vm(vm.get(marker)) {
            Queue::main().exec_after(Duration::from_secs(15), || force_stop_vm(vm));
        } else {
            force_stop_vm(vm);
        }
    });
}

fn request_stop_vm(vm: &Retained<VZVirtualMachine>) -> bool {
    unsafe {
        if vm.canRequestStop() {
            info!("request vm to stop");
            if let Err(err) = vm.requestStopWithError() {
                error!("failed to request vm to stop, error={}", err.localizedDescription());
                process::exit(1);
            }
            return true;
        }
        false
    }
}

fn force_stop_vm(vm: Arc<MainThreadBound<Retained<VZVirtualMachine>>>) {
    run_on_main(|marker| {
        info!("force to stop vm");
        let vm = vm.get(marker);
        if unsafe { vm.canStop() } {
            let block = &StackBlock::new(|err: *mut NSError| {
                if err.is_null() {
                    info!("vm stopped");
                    process::exit(0);
                } else {
                    error!("vm failed to stop, error={}", unsafe { (*err).localizedDescription() });
                    process::exit(1);
                }
            });
            unsafe {
                vm.stopWithCompletionHandler(block);
            }
        } else {
            process::exit(1);
        }
    });
}