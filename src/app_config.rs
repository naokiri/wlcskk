use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct AppConfig {
    pub(crate) version: i32,
    pub(crate) static_dictionary: Vec<DictionaryInfo>,
    pub(crate) user_dictionary: Vec<DictionaryInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct DictionaryInfo {
    pub(crate) path: String,
    pub(crate) encoding: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 0,
            static_dictionary: vec![
                DictionaryInfo {
                    path: "~/.local/share/wlcskk/dictionary/SKK-JISYO.L".into(),
                    encoding: "euc-jp".into(),
                },
                DictionaryInfo {
                    path: "~/.local/share/wlcskk/dictionary/SKK-JISYO.propernoun".into(),
                    encoding: "euc-jp".into(),
                },
            ],
            user_dictionary: vec![DictionaryInfo {
                path: "~/.local/share/wlcskk/dictionary/user.dict".into(),
                encoding: "utf-8".into(),
            }],
        }
    }
}
