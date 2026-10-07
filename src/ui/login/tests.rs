use super::{authenticate::authenticate, AuthMode};
use crate::{api::ApiClient, models::WhoamiResponse};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::{timeout, Duration},
};

const USER_ID: &str = "12345678-1234-4234-8234-123456789abc";

struct Step {
    method: &'static str,
    path: &'static str,
    status: u16,
    response: Value,
    raw_response: Option<&'static str>,
    content_type: &'static str,
    body: Option<Value>,
    cookie: Option<&'static str>,
    set_cookie: Option<&'static str>,
}

impl Step {
    fn get(path: &'static str, response: Value) -> Self {
        Self { method: "GET", path, status: 200, response,
            raw_response: None, content_type: "application/json",
            body: None, cookie: None, set_cookie: None }
    }

    fn post(path: &'static str, body: Value, response: Value) -> Self {
        Self { method: "POST", body: Some(body), ..Self::get(path, response) }
    }
}

fn server_step(private: bool) -> Step {
    Step::get("/server", json!({
        "id": USER_ID, "name": "Test server", "public": !private,
        "owner_id": null, "owner_username": null,
        "created_at": "2026-10-06T00:00:00Z"
    }))
}

fn login_step(cookie: Option<&'static str>) -> Step {
    Step {
        cookie,
        set_cookie: Some("Auth=session; Secure; HttpOnly; SameSite=Strict; Path=/"),
        ..Step::post("/auth/login", json!({"username": "alice", "password": "AccountPass!"}),
            json!({"user": {"id": USER_ID, "username": "alice"}}))
    }
}

fn whoami_step() -> Step {
    Step {
        cookie: Some("Auth=session"),
        ..Step::get("/auth/whoami", json!({
            "id": USER_ID, "username": "alice", "created_at": "2026-10-06T00:00:00Z"
        }))
    }
}

async fn mock_server(steps: Vec<Step>) -> (ApiClient, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    // Exercise DNS resolution of localhost against an IPv4-only listener.
    let client = ApiClient::new(&format!("http://localhost:{}", listener.local_addr().unwrap().port())).unwrap();
    let task = tokio::spawn(async move {
        for step in steps {
            let (mut stream, _) = timeout(Duration::from_secs(5), listener.accept()).await.unwrap().unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 2048];
            let (headers, body) = loop {
                let n = timeout(Duration::from_secs(5), stream.read(&mut buffer)).await.unwrap().unwrap();
                assert!(n > 0, "client closed before sending complete request");
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8(bytes[..end].to_vec()).unwrap();
                    let length = header(&headers, "content-length")
                        .map(|value| value.parse::<usize>().unwrap()).unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break (headers, bytes[end + 4..end + 4 + length].to_vec());
                    }
                }
            };
            assert_eq!(headers.lines().next().unwrap(), format!("{} {} HTTP/1.1", step.method, step.path));
            assert!(header(&headers, "host").unwrap().starts_with("localhost:"));
            assert_eq!(header(&headers, "cookie"), step.cookie);
            if let Some(expected) = step.body {
                assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(), expected);
            }
            let response = step.raw_response.map(str::to_owned).unwrap_or_else(|| step.response.to_string());
            let cookie = step.set_cookie.map(|cookie| format!("Set-Cookie: {cookie}\r\n")).unwrap_or_default();
            let body = if step.status == 204 { "" } else { &response };
            stream.write_all(format!(
                "HTTP/1.1 {} Test\r\nContent-Type: {}\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                step.status, step.content_type, cookie, body.len(), body
            ).as_bytes()).await.unwrap();
            stream.shutdown().await.unwrap();
        }
    });
    (client, task)
}

fn header<'a>(headers: &'a str, name: &str) -> Option<&'a str> {
    headers.lines().filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.trim())
}

async fn run_flow(client: &ApiClient, mode: AuthMode, server_password: &str) -> anyhow::Result<WhoamiResponse> {
    timeout(Duration::from_secs(5), authenticate(client, &mode, "alice", "AccountPass!", server_password))
        .await.expect("authentication test timed out")
}

#[tokio::test]
async fn localhost_public_login_uses_secure_cookie_for_profile_and_websocket() {
    let (client, server) = mock_server(vec![
        Step::get("/health", json!("OK")), server_step(false), login_step(None), whoami_step(),
    ]).await;
    assert_eq!(run_flow(&client, AuthMode::Login, "").await.unwrap().username, "alice");
    assert_eq!(client.websocket_request().unwrap().headers()["Cookie"], "Auth=session");
    server.await.unwrap();
}

#[tokio::test]
async fn private_login_and_registration_authorize_server_before_account() {
    for mode in [AuthMode::Login, AuthMode::Register] {
        let mut steps = vec![
            Step::get("/health", json!("OK")),
            Step {
                status: 204,
                set_cookie: Some("Auth=temporary; Secure; HttpOnly; SameSite=Strict; Path=/"),
                ..Step::post("/auth/login_server", json!({"server_password": " ServerPass! "}), Value::Null)
            },
        ];
        if mode == AuthMode::Register {
            steps.push(Step {
                status: 201, cookie: Some("Auth=temporary"),
                ..Step::post("/auth/register", json!({"username": "alice", "password": "AccountPass!"}), json!({}))
            });
        }
        steps.extend([login_step(Some("Auth=temporary")), whoami_step()]);
        let (client, server) = mock_server(steps).await;
        assert_eq!(run_flow(&client, mode, " ServerPass! ").await.unwrap().username, "alice");
        assert_eq!(client.websocket_request().unwrap().headers()["Cookie"], "Auth=session");
        server.await.unwrap();
    }
}

#[tokio::test]
async fn private_server_without_password_requests_it_before_account_login() {
    let (client, server) = mock_server(vec![Step::get("/health", json!("OK")), server_step(true)]).await;
    let error = run_flow(&client, AuthMode::Login, "").await.unwrap_err();
    assert!(error.to_string().contains("Preencha o campo Senha do servidor"));
    server.await.unwrap();
}

#[tokio::test]
async fn invalid_server_password_stops_before_registration() {
    let (client, server) = mock_server(vec![
        Step::get("/health", json!("OK")),
        Step {
            status: 401,
            ..Step::post("/auth/login_server", json!({"server_password": "wrong"}),
                json!({"detail": "senha do servidor incorreta"}))
        },
    ]).await;
    let error = run_flow(&client, AuthMode::Register, "wrong").await.unwrap_err();
    assert!(format!("{error:#}").contains("senha do servidor incorreta"));
    server.await.unwrap();
}

#[tokio::test]
async fn bootstrap_without_server_record_still_allows_account_login() {
    let (client, server) = mock_server(vec![
        Step::get("/health", json!("OK")),
        Step { status: 404, ..Step::get("/server", json!({"detail": "servidor não encontrado"})) },
        login_step(None), whoami_step(),
    ]).await;
    assert!(run_flow(&client, AuthMode::Login, "").await.is_ok());
    server.await.unwrap();
}

#[tokio::test]
async fn stopped_local_backend_explains_address_and_port() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = ApiClient::new(&format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port())).unwrap();
    drop(listener);
    let error = run_flow(&client, AuthMode::Login, "").await.unwrap_err().to_string();
    assert!(error.contains(client.base_url()));
    assert!(error.contains("Confirme que o backend está rodando"));
}

#[tokio::test]
async fn unhealthy_backend_reports_its_error_instead_of_connection_failure() {
    let (client, server) = mock_server(vec![Step {
        status: 503, ..Step::get("/health", json!({"detail": "Banco indisponível"}))
    }]).await;
    let error = format!("{:#}", run_flow(&client, AuthMode::Login, "").await.unwrap_err());
    assert!(error.contains("Banco indisponível"));
    assert!(!error.contains("Confirme que o backend está rodando"));
    server.await.unwrap();
}

#[tokio::test]
async fn website_html_health_response_stops_before_sending_credentials() {
    let (client, server) = mock_server(vec![Step {
        raw_response: Some("<!doctype html><html><title>Papo</title></html>"),
        content_type: "text/html",
        ..Step::get("/health", Value::Null)
    }]).await;
    let error = format!("{:#}", run_flow(&client, AuthMode::Login, "ServerPass!").await.unwrap_err());
    assert!(error.contains("página HTML"));
    assert!(error.contains("/health"));
    assert!(error.contains("endereço da API"));
    assert!(!error.contains("expected value"));
    server.await.unwrap();
}

#[tokio::test]
async fn non_json_server_metadata_has_actionable_diagnostics() {
    for (body, content_type, expected) in [
        ("<!doctype html><html>private page contents</html>", "text/html", "página HTML"),
        ("", "application/json", "Resposta vazia"),
        ("this is not JSON", "text/plain", "Resposta JSON inválida"),
    ] {
        let (client, server) = mock_server(vec![
            Step::get("/health", json!("OK")),
            Step { raw_response: Some(body), content_type, ..Step::get("/server", Value::Null) },
        ]).await;
        let error = format!("{:#}", run_flow(&client, AuthMode::Login, "").await.unwrap_err());
        assert!(error.contains(expected), "{error}");
        assert!(error.contains("/server"));
        assert!(error.contains("HTTP 200"));
        assert!(!error.contains("private page contents"));
        server.await.unwrap();
    }
}

#[tokio::test]
async fn server_password_endpoint_cannot_accept_html_as_success() {
    let (client, server) = mock_server(vec![
        Step::get("/health", json!("OK")),
        Step {
            raw_response: Some("<html>login page</html>"), content_type: "text/html",
            ..Step::post("/auth/login_server", json!({"server_password": "ServerPass!"}), Value::Null)
        },
    ]).await;
    let error = format!("{:#}", run_flow(&client, AuthMode::Login, "ServerPass!").await.unwrap_err());
    assert!(error.contains("Falha ao validar a senha do servidor"));
    assert!(error.contains("página HTML"));
    server.await.unwrap();
}

#[tokio::test]
async fn account_login_and_profile_reject_html_with_endpoint_context() {
    for endpoint in ["/auth/login", "/auth/whoami"] {
        let mut steps = vec![Step::get("/health", json!("OK")), server_step(false)];
        if endpoint == "/auth/whoami" {
            steps.push(login_step(None));
            steps.push(Step {
                cookie: Some("Auth=session"), raw_response: Some("<html>wrong route</html>"),
                ..Step::get("/auth/whoami", Value::Null)
            });
        } else {
            steps.push(Step { raw_response: Some("<html>wrong route</html>"), ..login_step(None) });
        }
        let (client, server) = mock_server(steps).await;
        let error = format!("{:#}", run_flow(&client, AuthMode::Login, "").await.unwrap_err());
        assert!(error.contains("página HTML"));
        assert!(error.contains(endpoint));
        server.await.unwrap();
    }
}

#[tokio::test]
async fn historical_connection_violation_warns_without_rejecting_a_fresh_login(){
    let mut login=login_step(None);login.response["connection_violation"]=json!(true);
    let(client,server)=mock_server(vec![Step::get("/health",json!("OK")),server_step(false),login,whoami_step()]).await;
    let user=run_flow(&client,AuthMode::Login,"").await.unwrap();assert_eq!(user.connection_violation,Some(true));server.await.unwrap();
}

/// Runs on the existing GTK thread and edits actual widgets rather than sending
/// synthetic SetPassword messages, so text/model signal feedback is exercised.
pub(crate) fn exercise_input(
    login: &relm4::Controller<super::LoginModel>,
    context: &gtk::glib::MainContext,
) {
    use adw::prelude::*;
    use relm4::ComponentController;
    use super::LoginMsg;
    use crate::ui::chat::actions::tests::{descendants, pump};
    use std::{cell::Cell, rc::Rc};

    let widgets = descendants(login.widget().upcast_ref());
    let entry = |title: &str| widgets.iter().find_map(|w| {
        w.downcast_ref::<adw::EntryRow>().filter(|row| row.title() == title).cloned()
    }).unwrap();
    let password = |title: &str| widgets.iter().find_map(|w| {
        w.downcast_ref::<adw::PasswordEntryRow>().filter(|row| row.title() == title).cloned()
    }).unwrap();
    let server = entry("Endereço do Servidor");
    let username = entry("Usuário");
    let account = password("Senha");
    let server_password = password("Senha do servidor (se necessário)");
    let changes = Rc::new(Cell::new(0));
    for row in [&account, &server_password] {
        let count = changes.clone();
        row.connect_changed(move |_| count.set(count.get() + 1));
    }

    server.set_text("http://localhost:8080");
    username.set_text("login-regression");
    pump(context);
    for row in [&account, &server_password] {
        row.set_text("");
        for character in "Abé!".chars() {
            let mut caret = row.text().chars().count() as i32;
            row.insert_text(&character.to_string(), &mut caret);
            row.set_position(caret);
            pump(context);
            assert!(changes.get() < 50, "login text changes must not form a feedback loop");
        }
        assert_eq!(row.text(), "Abé!");
    }
    assert_eq!(login.model().password, "Abé!");
    assert_eq!(login.model().server_password, "Abé!");

    // An unrelated form update must leave password text and selection alone.
    account.select_region(1, 3);
    let selection = account.selection_bounds();
    let before = changes.get();
    username.set_text("edited-name");
    server.set_text("http://127.0.0.1:8080");
    pump(context);
    assert_eq!(account.selection_bounds(), selection);
    assert_eq!(changes.get(), before);
    assert_eq!(login.model().username, "edited-name");
    assert_eq!(login.model().server_url, "http://127.0.0.1:8080");

    let mut caret = 1;
    account.insert_text("XYZ", &mut caret);
    account.set_position(caret);
    pump(context);
    assert_eq!(account.text(), "AXYZbé!");
    assert_eq!(account.position(), 4);
    assert_eq!(login.model().password, "AXYZbé!");
    account.delete_text(1, 4);
    pump(context);
    assert_eq!(account.text(), "Abé!");
    assert_eq!(login.model().password, "Abé!");
    // Several edits can queue before the component catches up. It must not
    // rewrite earlier snapshots into the entry and push the caret to the end.
    account.set_position(1);
    let mut caret = 1;
    for character in "XYZ".chars() {
        account.insert_text(&character.to_string(), &mut caret);
        account.set_position(caret);
    }
    pump(context);
    assert_eq!(account.text(), "AXYZbé!");
    assert_eq!(account.position(), 4);
    assert_eq!(login.model().password, "AXYZbé!");
    login.emit(LoginMsg::AuthFailed("test failure".into()));
    login.emit(LoginMsg::ToggleMode);
    pump(context);
    assert_eq!(account.text(), "AXYZbé!");
    assert_eq!(server_password.text(), "Abé!");

    login.emit(LoginMsg::ClearCredentials);
    pump(context);
    assert!(account.text().is_empty() && server_password.text().is_empty());
    assert!(login.model().password.is_empty() && login.model().server_password.is_empty());
    assert!(changes.get() < 50, "clearing credentials must not enqueue recurring edits");
    login.emit(LoginMsg::ToggleMode);
    pump(context);
}
