//! EN-US: Retained but undisplayed metadata must keep its original wire validation.
//! PT-BR: Metadados preservados, mas não exibidos, mantêm a validação original do formato de rede.
use super::*;
use uuid::Uuid;
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};

fn validates<T: DeserializeOwned>(valid: Value, invalid: &[(&str, Value)], required: &[&str]) {
    assert!(serde_json::from_value::<T>(valid.clone()).is_ok());
    for (field, value) in invalid {
        let mut changed = valid.clone(); changed[*field] = value.clone();
        assert!(serde_json::from_value::<T>(changed).is_err(), "invalid {field} must not be ignored");
    }
    for field in required {
        let mut changed = valid.clone(); changed.as_object_mut().unwrap().remove(*field);
        assert!(serde_json::from_value::<T>(changed).is_err(), "required {field} must not become optional");
    }
}

#[test]
fn undisplayed_response_fields_still_validate_ids_timestamps_and_counts() {
    let id = Uuid::nil(); let date = "2026-10-15T12:00:00Z";
    validates::<RegisterResponse>(json!({}), &[("id",json!("not-a-uuid")),("created_at",json!("yesterday")),("username",json!(5))], &[]);
    validates::<Server>(json!({"id":id,"name":"Test","created_at":date}), &[("created_at",json!("yesterday")),("member_count",json!("many"))], &["created_at"]);
    validates::<UserSettings>(json!({"user_id":id,"version":1,"config":{},"updated_at":date}), &[("updated_at",json!("yesterday"))], &["updated_at"]);
    validates::<Webhook>(json!({"id":id,"channel_id":id,"name":"Bridge","created_by":id,"created_at":date}), &[("created_by",json!("not-a-uuid")),("created_at",json!("yesterday")),("revoked_at",json!(7))], &["created_by","created_at"]);
    validates::<UserRole>(json!({"user_id":id,"role_id":id,"assigned_at":date}), &[("role_id",json!("not-a-uuid")),("assigned_at",json!(false))], &["user_id","role_id","assigned_at"]);
}

#[test]
fn undisplayed_event_metadata_remains_required_and_typed() {
    let id = Uuid::nil();
    for kind in ["role_add","role_remove","message_delete"] {
        let mut value = json!({"type":kind,"user_id":id,"role_id":id,"id":id,"channel_id":id});
        assert!(crate::ws::parse_event(&value.to_string()).is_some());
        let key = if kind=="message_delete" {"channel_id"} else {"role_id"};
        value[key]=json!("not-a-uuid"); assert!(crate::ws::parse_event(&value.to_string()).is_none());
        value.as_object_mut().unwrap().remove(key); assert!(crate::ws::parse_event(&value.to_string()).is_none());
    }
    // Decode without reading these fields: the field-local expectations must
    // not remove metadata or relax the native worker's JSON contract.
    for value in [json!({"type":"started","pid":123}),json!({"type":"microphone_gain","gain":1.5}),json!({"type":"track","track_id":"remote-a"})] {
        let mut invalid=value.clone();let key=if value["type"]=="started" {"pid"} else if value["type"]=="track" {"track_id"} else {"gain"};
        assert!(crate::voice::EngineEvent::deserialize(value).is_ok());
        invalid[key]=json!([]);assert!(crate::voice::EngineEvent::deserialize(invalid).is_err());
    }
}
