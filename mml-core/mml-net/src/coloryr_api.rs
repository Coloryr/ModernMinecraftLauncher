//! ColorYr 自有服务接口，用于 MCMod 查询
//!
//! 服务端尚未启用，代码整体保留为注释，恢复时直接解开即可。

// use std::collections::HashMap;

// use mml_names::i18_items::error_type::ErrorType;
// use serde::{Deserialize, Serialize};

// #[derive(Serialize, Deserialize, Debug)]
// #[serde(default)]
// pub struct McModSearchItemObj {
//     pub mcmod_id: i32,
//     pub mcmod_icon: String,
//     pub mcmod_name: String,
//     pub mcmod_author: String,
//     pub mcmod_sub: String,
//     pub mcmod_abbr: String,
//     pub mcmod_modid: String,
//     pub mcmod_type: i32,
//     pub curseforge_url: Option<String>,
//     pub curseforge_id: Option<String>,
//     pub modrinth_url: Option<String>,
//     pub modrinth_id: Option<String>,
// }

// impl Default for McModSearchItemObj {
//     fn default() -> Self {
//         Self {
//             mcmod_id: 0,
//             mcmod_icon: String::new(),
//             mcmod_name: String::new(),
//             mcmod_author: String::new(),
//             mcmod_sub: String::new(),
//             mcmod_abbr: String::new(),
//             mcmod_modid: String::new(),
//             mcmod_type: 0,
//             curseforge_url: None,
//             curseforge_id: None,
//             modrinth_url: None,
//             modrinth_id: None,
//         }
//     }
// }

// #[derive(Serialize, Deserialize, Debug)]
// #[serde(default)]
// pub struct McModSearchObj {
//     #[serde(rename = "type")]
//     pub mtype: i32,
//     pub ids: Vec<String>,
//     pub mcmod_type: i32,
// }

// impl Default for McModSearchObj {
//     fn default() -> Self {
//         Self {
//             mtype: 0,
//             ids: Vec::new(),
//             mcmod_type: 0,
//         }
//     }
// }

// #[derive(Serialize, Deserialize, Debug)]
// #[serde(default)]
// pub struct McModSearchResObj {
//     pub res: i32,
//     pub data: Option<HashMap<String, McModSearchItemObj>>,
// }

// impl Default for McModSearchResObj {
//     fn default() -> Self {
//         Self { res: 0, data: None }
//     }
// }

// #[derive(Serialize, Deserialize, Debug)]
// #[serde(default)]
// pub struct McModTypesObj {
//     pub types: Vec<String>,
//     pub sorts: Vec<String>,
//     pub versions: Vec<String>,
// }

// impl Default for McModTypesObj {
//     fn default() -> Self {
//         Self {
//             types: Vec::new(),
//             sorts: Vec::new(),
//             versions: Vec::new(),
//         }
//     }
// }

// #[derive(Serialize, Deserialize, Debug)]
// #[serde(default)]
// pub struct McModTypsResObj {
//     pub res: i32,
//     pub data: Option<McModTypesObj>,
// }

// impl Default for McModTypsResObj {
//     fn default() -> Self {
//         Self { res: 0, data: None }
//     }
// }

// const COLORYR_URL: &str = "https://api.coloryr.com:8081/";

// async fn get_list(
//     mtype: i32,
//     ids: Vec<String>,
//     mcmod_type: i32,
// ) -> Result<HashMap<String, McModSearchItemObj>, ErrorType> {
//     let mut url = String::from(COLORYR_URL);
//     url.push_str("findmod");

//     let send = McModSearchObj {
//         mcmod_type,
//         mtype,
//         ids,
//     };

//     let data = mml_http::WORK_CLIENT
//         .get()
//         .unwrap()
//         .post_json_get_json::<_, McModSearchResObj>(url.as_str(), &send)
//         .await;

//     match data {
//         Ok(data) => {
//             if data.res != 100 {
//                 Err(ErrorType::HttpError(data.res))
//             } else {
//                 Ok(data.data.unwrap())
//             }
//         }
//         Err(err) => Err(ErrorType::ColoryrApiGetError(err.to_string())),
//     }
// }

// pub async fn get_mcmod_from_cf(
//     ids: Vec<String>,
//     mcmod_type: i32,
// ) -> Result<HashMap<String, McModSearchItemObj>, ErrorType> {
//     get_list(0, ids, mcmod_type).await
// }

// pub async fn get_mcmod_from_mo(
//     ids: Vec<String>,
//     mcmod_type: i32,
// ) -> Result<HashMap<String, McModSearchItemObj>, ErrorType> {
//     get_list(1, ids, mcmod_type).await
// }

// pub async fn get_mcmod(name: String, page:i32, loader: i32) {

// }
