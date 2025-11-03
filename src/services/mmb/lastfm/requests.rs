// use serde::Deserialize;
// use smallvec::SmallVec;

// pub struct LFMRequestBuilder {
//     api_key: String,
//     params: SmallVec<[(&'static str, String); 5]>,
//     endpoint: String,
//     signature: Option<String>,
//     read: bool,
// }

// impl LFMRequestBuilder {
//     pub fn new(api_key: String) -> Self {
//         LFMRequestBuilder {
//             api_key,
//             params: SmallVec::new(),
//             endpoint: "https://ws.audioscrobbler.com/2.0/?format=json".to_string(),
//             signature: None,
//             read: true,
//         }
//     }

//     pub fn set_endpoint(mut self, endpoint: String) -> Self {
//         self.endpoint = endpoint;
//         self
//     }

//     pub fn add_param(mut self, key: &'static str, value: String) -> Self {
//         if self.signature.is_none() {
//             self.params.push((key, value));
//         } else {
//             panic!("cannot add params after signing");
//         }

//         self
//     }

//     pub fn add_optional_param(mut self, key: &'static str, value: Option<String>) -> Self {
//         if self.signature.is_none() {
//             if let Some(value) = value {
//                 self.params.push((key, value));
//             }
//         } else {
//             panic!("cannot add params after signing");
//         }

//         self
//     }

//     pub fn sign(mut self, secret: &str) -> Self {
//         self.params.insert(0, ("api_key", self.api_key.clone()));

//         self.params.sort_by(|a, b| a.0.cmp(b.0));

//         let params = self.params.clone();
//         let mut sig = String::new();
//         for (k, v) in params.iter() {
//             sig.push_str(k);
//             sig.push_str(v);
//         }
//         sig.push_str(secret);
//         self.signature = Some(format!("{:x}", md5::compute(sig)));

//         self
//     }

//     pub async fn send_request<T: for<'de> Deserialize<'de>>(self, client: zed_reqwest::Client) -> anyhow::Result<T> {
//         if self.read {
//             self.send_read_request::<T>(client).await
//         } else {
//             self.send_write_request::<T>(client).await
//         }
//     }

//     pub fn read(mut self) -> Self {
//         self.read = true;
//         self
//     }

//     pub fn write(mut self) -> Self {
//         self.read = false;
//         self
//     }

//     async fn send_read_request<T: for<'de> Deserialize<'de>>(self, client: zed_reqwest::Client) -> anyhow::Result<T> {
//         let mut url = self.endpoint.clone();
//         url.push('&');

//         for (k, v) in self.params.iter() {
//             url.push_str(k);
//             url.push('=');
//             url.push_str(v);
//             url.push('&');
//         }
//         zed_reqwest::Request::new()

//         url.push_str("api_sig=");
//         url.push_str(
//             self.signature
//                 .as_ref()
//                 .ok_or(anyhow::Error::msg("couldn't unwrap signature"))?,
//         );

//         let mut response = isahc::get_async(url).await?;
//         let body = response.text().await?;
//         serde_json::from_str(&body).map_err(anyhow::Error::from)
//     }

//     pub async fn send_write_request_ns(self, client: zed_reqwest::Client) -> anyhow::Result<String> {
//         // URL encode the parameters for the POST body
//         let mut body = String::new();

//         for (k, v) in self.params.iter() {
//             body.push_str(k);
//             body.push('=');
//             body.push_str(&urlencoding::encode(v));
//             body.push('&');
//         }

//         body.push_str("api_sig=");
//         body.push_str(
//             self.signature
//                 .as_ref()
//                 .ok_or(anyhow::Error::msg("couldn't unwrap signature"))?,
//         );

//         let mut response = isahc::post_async(self.endpoint, body).await?;
//         let body = response.text().await?;

//         Ok(body)
//     }

//     // async fn send_write_request<T: for<'de> Deserialize<'de>>(self) -> anyhow::Result<T> {
//     //     let body = self.send_write_request_ns().await?;
//     //     serde_json::from_str(&body).map_err(anyhow::Error::from)
//     // }
// }
