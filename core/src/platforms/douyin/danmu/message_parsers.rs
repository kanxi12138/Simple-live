use super::gen::{ChatMessage, LikeMessage, MemberMessage, RoomStatsMessage}; // Updated to directly use types from gen
use crate::platforms::common::DanmakuFrontendPayload;
use prost::Message as ProstMessage; // For .decode() // Use shared payload type

// Parser for ChatMessage
pub fn parse_chat_message(
    payload: &[u8],
    current_room_id: &str,
) -> Result<Option<DanmakuFrontendPayload>, Box<dyn std::error::Error + Send + Sync>> {
    match ChatMessage::decode(payload) {
        Ok(chat_msg) => {
            if let Some(user) = chat_msg.user {
                // 获取用户等级 (来自 demo)
                let user_level = user.pay_grade.as_ref().map(|pg| pg.level).unwrap_or(0);
                // 获取粉丝牌等级 (来自 demo)
                // 注意：demo中的 fans_club.data.level 路径，确保你的 proto 定义一致
                // 如果你的 User 结构体中 fans_club 直接是 FansClubData 类型，则不需要 .data
                // 假设 FansClub 结构体包含一个 Option<FansClubData> 类型的字段 data
                let fans_club_level = user
                    .fans_club
                    .as_ref()
                    .and_then(|fc| fc.data.as_ref()) // 如果 FansClub 直接是 FansClubData，则为 .map(|fc| fc.level)
                    .map(|fcd| fcd.level)
                    .unwrap_or(0);

                Ok(Some(DanmakuFrontendPayload {
                    room_id: current_room_id.to_string(), // Populate room_id
                    user: user.nick_name.clone(),
                    content: chat_msg.content.clone(),
                    user_level,
                    fans_club_level,
                    // r#type: "chat".to_string(),
                }))
            } else {
                // 对于没有用户信息的聊天消息 (例如系统消息)，也可能需要发送，但等级为0
                println!("Diagnostic: message_parsers.rs:36 (details omitted)");
                Ok(Some(DanmakuFrontendPayload {
                    room_id: current_room_id.to_string(), // Populate room_id
                    user: "系统".to_string(),             // Or some other placeholder
                    content: chat_msg.content.clone(),
                    user_level: 0,
                    fans_club_level: 0,
                }))
            }
        }
        Err(e) => {
            // eprintln!("Diagnostic: message_parsers.rs:50 (details omitted)"); // Commented out to suppress error logging as per user request
            Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }
}

// Demo 中此函数返回 Result<(), ...> 并且只打印，这里保持原有返回 Option<DanmakuFrontendPayload> 结构
// 如果不需要将进场消息发送到前端，可以保持返回 Ok(None)
#[allow(dead_code)] // ADDED to suppress warning
pub fn parse_member_message(
    payload: &[u8],
    _current_room_id: &str,
) -> Result<Option<DanmakuFrontendPayload>, Box<dyn std::error::Error + Send + Sync>> {
    match MemberMessage::decode(payload) {
        Ok(member_msg) => {
            if member_msg.user.is_some() {

                println!("Diagnostic: message_parsers.rs:74 (details omitted)");
                Ok(None) // 当前不发送到前端
            } else {
                println!("Diagnostic: message_parsers.rs:80 (details omitted)");
                Ok(None)
            }
        }
        Err(e) => {
            eprintln!("Diagnostic: message_parsers.rs:85 (details omitted)");
            Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }
}

// Parser for LikeMessage (点赞消息)
// Demo 中此函数返回 Result<(), ...> 并且只打印
#[allow(dead_code)] // ADDED to suppress warning
pub fn parse_like_message(
    payload: &[u8],
    _current_room_id: &str,
) -> Result<Option<DanmakuFrontendPayload>, Box<dyn std::error::Error + Send + Sync>> {
    match LikeMessage::decode(payload) {
        Ok(like_msg) => {
            if like_msg.user.is_some() {
                println!("Diagnostic: message_parsers.rs:101 (details omitted)");
            } else {
                println!("Diagnostic: message_parsers.rs:106 (details omitted)");
            }
            Ok(None) // 点赞消息通常不直接作为弹幕显示在列表
        }
        Err(e) => {
            eprintln!("Diagnostic: message_parsers.rs:111 (details omitted)");
            Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }
}

#[allow(dead_code)] // ADDED to suppress warning
pub fn parse_room_stats_message(
    payload: &[u8],
    _current_room_id: &str,
) -> Result<Option<DanmakuFrontendPayload>, Box<dyn std::error::Error + Send + Sync>> {
    match RoomStatsMessage::decode(payload) {
        Ok(_stats_msg) => {
            println!("Diagnostic: message_parsers.rs:124 (details omitted)");
            Ok(None) // 统计信息通常不作为普通弹幕显示
        }
        Err(e) => {
            eprintln!("Diagnostic: message_parsers.rs:128 (details omitted)");
            Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    }
}
