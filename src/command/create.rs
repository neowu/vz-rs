use std::cmp::max;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::channel;

use block2::StackBlock;
use clap::Args;
use clap::ValueHint;
use objc2::exception::catch;
use objc2::rc::Id;
use objc2::rc::Retained;
use objc2::ClassType;
use objc2_foundation::NSDataBase64EncodingOptions;
use objc2_foundation::NSError;
use objc2_virtualization::VZEFIVariableStore;
use objc2_virtualization::VZEFIVariableStoreInitializationOptions;
use objc2_virtualization::VZMACAddress;
use objc2_virtualization::VZMacAuxiliaryStorage;
use objc2_virtualization::VZMacAuxiliaryStorageInitializationOptions;
use objc2_virtualization::VZMacMachineIdentifier;
use objc2_virtualization::VZMacOSRestoreImage;
use tracing::info;

use crate::config::vm_config::Os;
use crate::config::vm_config::VmConfig;
use crate::config::vm_dir;
use crate::config::vm_dir::VmDir;
use crate::util::exception::Exception;
use crate::util::path::PathExtension;
use crate::vm::mac_os;

#[derive(Args)]
pub struct Create {
    #[arg(help = "vm name")]
    name: String,

    #[arg(long, help = "create a linux or macOS vm", default_value = "linux")]
    os: Os,

    #[arg(long, help = "disk size in gb", default_value_t = 50)]
    disk_size: u64,

    #[arg(long, help = "macOS restore image file, e.g. --ipsw=UniversalMac_14.5_23F79_Restore.ipsw", value_hint = ValueHint::FilePath)]
    ipsw: Option<PathBuf>,
}

impl Create {
    pub fn execute(&self) -> Result<(), Exception> {
        self.validate()?;

        let name = &self.name;
        let dir = vm_dir::vm_dir(name);
        if dir.initialized() {
            return Err(Exception::ValidationError(format!("vm already exists, name={name}")));
        }

        let temp_dir = vm_dir::create_temp_vm_dir()?;
        temp_dir.resize(self.disk_size * 1_000_000_000)?;

        match self.os {
            Os::Linux => create_linux(&temp_dir)?,
            Os::MacOs => create_macos(&temp_dir, &self.ipsw.as_ref().unwrap().to_absolute_path())?,
        }

        let dir = vm_dir::vm_dir(&self.name);
        info!("move vm dir, from={}, to={}", temp_dir.dir.to_string_lossy(), dir.dir.to_string_lossy());
        fs::rename(&temp_dir.dir, &dir.dir)?;
        info!("vm created, name={}, config={}", self.name, dir.config_path.to_string_lossy());

        Ok(())
    }

    pub fn validate(&self) -> Result<(), Exception> {
        if let Os::MacOs = self.os {
            match &self.ipsw {
                Some(path) => {
                    if !path.exists() {
                        return Err(Exception::ValidationError(format!(
                            "ipsw does not exist, path={}",
                            path.to_string_lossy()
                        )));
                    }
                }
                None => return Err(Exception::ValidationError("ipsw is required for macOS vm".to_string())),
            }
        };
        Ok(())
    }
}

fn create_linux(dir: &VmDir) -> Result<(), Exception> {
    info!("create nvram.bin");
    unsafe {
        catch(|| {
            VZEFIVariableStore::initCreatingVariableStoreAtURL_options_error(
                VZEFIVariableStore::alloc(),
                &dir.nvram_path.to_ns_url(),
                VZEFIVariableStoreInitializationOptions::empty(),
            )
        })??;
    }

    info!("create config.json");
    let config = VmConfig {
        os: Os::Linux,
        cpu: 1,
        memory: 1024 * 1024 * 1024,
        mac_address: random_mac_address(),
        sharing: HashMap::new(),
        rosetta: Some(false),
        hardware_model: None,
        machine_identifier: None,
    };
    dir.save_config(&config)?;

    Ok(())
}

fn create_macos(dir: &VmDir, ipsw: &Path) -> Result<(), Exception> {
    let image = load_mac_os_restore_image(ipsw)?;

    let requirements = unsafe {
        image
            .mostFeaturefulSupportedConfiguration()
            .ok_or_else(|| Exception::ValidationError("restore image is not supported by current host".to_string()))
    }?;

    info!("create nvram.bin");
    let hardware_model = unsafe {
        requirements
            .hardwareModel()
            .dataRepresentation()
            .base64EncodedStringWithOptions(NSDataBase64EncodingOptions::empty())
            .to_string()
    };
    unsafe {
        catch(|| {
            let model = mac_os::hardware_model(&hardware_model);
            VZMacAuxiliaryStorage::initCreatingStorageAtURL_hardwareModel_options_error(
                VZMacAuxiliaryStorage::alloc(),
                &dir.nvram_path.to_ns_url(),
                &model,
                VZMacAuxiliaryStorageInitializationOptions::empty(),
            )
        })??;
    }

    info!("create config.json");
    let machine_identifier = unsafe {
        VZMacMachineIdentifier::new()
            .dataRepresentation()
            .base64EncodedStringWithOptions(NSDataBase64EncodingOptions::empty())
            .to_string()
    };
    let config = VmConfig {
        os: Os::MacOs,
        cpu: max(4, unsafe { requirements.minimumSupportedCPUCount() }),
        memory: max(8 * 1024 * 1024 * 1024, unsafe { requirements.minimumSupportedMemorySize() }),
        mac_address: random_mac_address(),
        sharing: HashMap::new(),
        rosetta: None,
        hardware_model: Some(hardware_model),
        machine_identifier: Some(machine_identifier),
    };
    dir.save_config(&config)?;

    Ok(())
}

fn random_mac_address() -> String {
    unsafe { VZMACAddress::randomLocallyAdministeredAddress().string().to_string() }
}

fn load_mac_os_restore_image(ipsw: &Path) -> Result<Retained<VZMacOSRestoreImage>, Exception> {
    let (tx, rx) = channel();
    unsafe {
        let block = StackBlock::new(move |image: *mut VZMacOSRestoreImage, err: *mut NSError| {
            if !err.is_null() {
                tx.send(Err(Exception::from_ns_error(err))).unwrap();
            } else {
                let image = Id::from_raw(image).unwrap();
                tx.send(Ok(image)).unwrap();
            }
        });
        VZMacOSRestoreImage::loadFileURL_completionHandler(&ipsw.to_ns_url(), &block);
    };
    let image = rx.recv()??;
    Ok(image)
}
