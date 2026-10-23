//! 用本机认证服务检查刷新请求中的角色绑定，不连接外部登录服务。

use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

use mml_auth::legacy::{GuiSelectHandel, authlib_injector};
use mml_names::i18_items::error_type::ErrorType;
use serde_json::{Value, json};

struct Select(i32);

impl GuiSelectHandel for Select {
    fn select_auth(&self, auths: Vec<String>) -> i32 {
        assert_eq!(auths, ["Alpha", "Beta"]);
        self.0
    }
}

fn serve(responses: Vec<Value>) -> (String, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                if let Ok((stream, _)) = listener.accept() {
                    break stream;
                }
                assert!(Instant::now() < deadline, "未收到预期的认证请求");
                thread::sleep(Duration::from_millis(10));
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                let mut byte = [0u8];
                stream.read_exact(&mut byte).unwrap();
                head.push(byte[0]);
            }
            let head = String::from_utf8(head).unwrap().to_lowercase();
            let length: usize = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .unwrap()
                .parse()
                .unwrap();
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            requests.push(serde_json::from_slice(&body).unwrap());
            let body = response.to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
        requests
    });
    (url, handle)
}

#[tokio::test]
async fn authlib_binds_local_profile_selection_and_honors_cancel() {
    let root = mml_testutil::temp_path(format!("authlib-profiles-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    mml_log::start(&root).unwrap();
    mml_base::init(&root);
    mml_config::init(&root).unwrap();
    mml_config::write_config().http.login_proxy = mml_config::config_obj::ProxyState::None;
    mml_net::init();

    let profiles = [
        json!({"name": "Alpha", "id": "00000000000000000000000000000001"}),
        json!({"name": "Beta", "id": "00000000000000000000000000000002"}),
    ];
    // 手选、默认首项、用户名匹配、唯一角色、服务器已选角色。
    for (choice, user, available, selected, expected) in [
        (Some(1), "email", profiles.to_vec(), None, 1),
        (None, "email", profiles.to_vec(), None, 0),
        (None, "Beta", profiles.to_vec(), None, 1),
        (None, "email", vec![profiles[1].clone()], None, 1),
        (
            None,
            "email",
            profiles.to_vec(),
            Some(profiles[1].clone()),
            1,
        ),
    ] {
        let already_bound = selected.is_some();
        let authenticate = json!({"accessToken": "test-access", "clientToken": "test-client", "availableProfiles": available, "selectedProfile": selected});
        let refresh = json!({"accessToken": "test-refreshed", "clientToken": "test-client", "selectedProfile": profiles[expected]});
        let (url, handle) = serve(vec![authenticate, refresh]);
        let auth = authlib_injector::authenticate(
            "test-client".into(),
            user.into(),
            "test-password".into(),
            url,
            choice.map(|index| Box::new(Select(index)) as Box<dyn GuiSelectHandel>),
        )
        .await
        .unwrap();
        let requests = handle.join().unwrap();
        assert_eq!(
            requests[1]["selectedProfile"],
            if already_bound {
                Value::Null
            } else {
                profiles[expected].clone()
            }
        );
        assert_eq!(auth.uuid, profiles[expected]["id"].as_str().unwrap());
        assert_eq!(auth.access_token, "test-refreshed");
    }

    for choice in [-1, 2] {
        let (url, handle) = serve(vec![
            json!({"accessToken": "test-access", "clientToken": "test-client", "availableProfiles": profiles}),
        ]);
        let result = authlib_injector::authenticate(
            "test-client".into(),
            "email".into(),
            "test-password".into(),
            url,
            Some(Box::new(Select(choice))),
        )
        .await;
        assert!(matches!(result, Err(ErrorType::TaskCancel)));
        assert_eq!(handle.join().unwrap().len(), 1);
    }
}
