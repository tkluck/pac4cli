use async_trait::async_trait;

use crate::wpad;

pub struct NetworkManager {
}

impl NetworkManager {
    pub fn new() -> Self {
        NetworkManager {}
    }
}

#[async_trait]
impl wpad::NetworkEnvironment for NetworkManager {
    async fn get_wpad_info(&self) -> Result<wpad::WPADInfo, ()> {
        Ok(
            wpad::WPADInfo {
                wpad_option: None,
                domains: Vec::new(),
            }
        )
    }
}
