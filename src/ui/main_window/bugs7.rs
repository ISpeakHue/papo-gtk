//! Focused mapped profile, security and automatic notification acknowledgement checks.
use super::*;
use crate::ui::chat::actions::tests::{mock,pump,until};
pub(crate) fn exercise(context:&gtk::glib::MainContext){
    let (api,backend,server)=tokio::runtime::Handle::current().block_on(mock());
    let current_user=serde_json::from_value(serde_json::json!({"id":"12345678-1234-4234-8234-123456789abc","username":"alice","created_at":"2026-10-03T00:00:00Z"})).unwrap();
    let main=MainWindowModel::builder().launch(MainWindowInit{current_user,api_client:api}).detach();
    until(context,||!main.model().channels.is_empty()&&main.model().access_request.is_none());
    account::exercise_preferences_and_profiles(&main,context);
    security::exercise(&main,context);
    notifications::exercise(&main,context,&backend);
    drop(main);server.abort();pump(context);
}
