use chrono::{TimeZone, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// Mac Absolute Time epoch: 2001-01-01 00:00:00 UTC
const MAC_EPOCH_OFFSET: i64 = 978307200;

/// Convert macOS timestamp (nanoseconds since 2001-01-01) to Unix timestamp
fn mac_timestamp_to_unix(mac_ts: i64) -> i64 {
    // macOS High Sierra+ uses nanoseconds
    let seconds = mac_ts / 1_000_000_000;
    seconds + MAC_EPOCH_OFFSET
}

/// Get the path to the iMessage database
fn get_imessage_db_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join("Library/Messages/chat.db"))
}

/// Get the path to the AddressBook database
fn get_addressbook_db_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let sources_dir = home.join("Library/Application Support/AddressBook/Sources");

    // Find the first source directory with an AddressBook database
    if let Ok(entries) = std::fs::read_dir(&sources_dir) {
        for entry in entries.flatten() {
            let db_path = entry.path().join("AddressBook-v22.abcddb");
            if db_path.exists() {
                return Some(db_path);
            }
        }
    }

    // Fallback to direct path
    let direct_path = home.join("Library/Application Support/AddressBook/AddressBook-v22.abcddb");
    if direct_path.exists() {
        return Some(direct_path);
    }

    None
}

/// Normalize phone number for comparison (remove formatting)
fn normalize_phone(phone: &str) -> String {
    phone.chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .take(10) // Last 10 digits
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

/// Get contact name mappings from AddressBook
fn get_contact_names() -> HashMap<String, String> {
    let mut names: HashMap<String, String> = HashMap::new();

    let db_path = match get_addressbook_db_path() {
        Some(p) => p,
        None => return names,
    };

    let conn = match Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(c) => c,
        Err(_) => return names,
    };

    // Query for phone numbers
    let phone_query = "
        SELECT ZABCDRECORD.ZFIRSTNAME, ZABCDRECORD.ZLASTNAME, ZABCDPHONENUMBER.ZFULLNUMBER
        FROM ZABCDRECORD
        LEFT JOIN ZABCDPHONENUMBER ON ZABCDRECORD.Z_PK = ZABCDPHONENUMBER.ZOWNER
        WHERE ZABCDPHONENUMBER.ZFULLNUMBER IS NOT NULL
    ";

    if let Ok(mut stmt) = conn.prepare(phone_query) {
        if let Ok(rows) = stmt.query_map([], |row| {
            let first: Option<String> = row.get(0).ok();
            let last: Option<String> = row.get(1).ok();
            let phone: String = row.get(2)?;
            Ok((first, last, phone))
        }) {
            for row in rows.flatten() {
                let (first, last, phone) = row;
                let name = match (first, last) {
                    (Some(f), Some(l)) => format!("{} {}", f, l),
                    (Some(f), None) => f,
                    (None, Some(l)) => l,
                    (None, None) => continue,
                };

                // Store both normalized and original
                let normalized = normalize_phone(&phone);
                if !normalized.is_empty() {
                    names.insert(normalized.clone(), name.clone());
                    // Also store with +1 prefix variations
                    names.insert(format!("+1{}", normalized), name.clone());
                }
                names.insert(phone, name);
            }
        }
    }

    // Query for email addresses
    let email_query = "
        SELECT ZABCDRECORD.ZFIRSTNAME, ZABCDRECORD.ZLASTNAME, ZABCDEMAILADDRESS.ZADDRESS
        FROM ZABCDRECORD
        LEFT JOIN ZABCDEMAILADDRESS ON ZABCDRECORD.Z_PK = ZABCDEMAILADDRESS.ZOWNER
        WHERE ZABCDEMAILADDRESS.ZADDRESS IS NOT NULL
    ";

    if let Ok(mut stmt) = conn.prepare(email_query) {
        if let Ok(rows) = stmt.query_map([], |row| {
            let first: Option<String> = row.get(0).ok();
            let last: Option<String> = row.get(1).ok();
            let email: String = row.get(2)?;
            Ok((first, last, email))
        }) {
            for row in rows.flatten() {
                let (first, last, email) = row;
                let name = match (first, last) {
                    (Some(f), Some(l)) => format!("{} {}", f, l),
                    (Some(f), None) => f,
                    (None, Some(l)) => l,
                    (None, None) => continue,
                };
                names.insert(email.to_lowercase(), name);
            }
        }
    }

    names
}

/// Look up a contact name by phone/email
fn lookup_contact_name(identifier: &str, contacts: &HashMap<String, String>) -> Option<String> {
    // Try direct lookup
    if let Some(name) = contacts.get(identifier) {
        return Some(name.clone());
    }

    // Try lowercase for email
    if let Some(name) = contacts.get(&identifier.to_lowercase()) {
        return Some(name.clone());
    }

    // Try normalized phone lookup
    let normalized = normalize_phone(identifier);
    if let Some(name) = contacts.get(&normalized) {
        return Some(name.clone());
    }

    None
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Contact {
    pub id: i64,
    pub identifier: String,      // Phone number or email
    pub display_name: Option<String>,
    pub message_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Chat {
    pub id: i64,
    pub chat_identifier: String,
    pub display_name: Option<String>,
    pub is_group: bool,
    pub participant_count: i64,
    pub message_count: i64,
    pub participants: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: i64,
    pub text: Option<String>,
    pub date: i64,               // Unix timestamp
    pub date_formatted: String,
    pub is_from_me: bool,
    pub handle_id: i64,
    pub contact_identifier: String,
    pub chat_id: Option<i64>,
    pub has_attachment: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatStats {
    pub total_messages: i64,
    pub messages_sent: i64,
    pub messages_received: i64,
    pub total_contacts: i64,
    pub date_range_start: Option<i64>,
    pub date_range_end: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportOptions {
    pub start_date: Option<i64>,  // Unix timestamp
    pub end_date: Option<i64>,    // Unix timestamp
    pub contact_ids: Option<Vec<i64>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseStatus {
    pub accessible: bool,
    pub path: String,
    pub error: Option<String>,
}

/// Check if we can access the iMessage database (Full Disk Access required)
#[tauri::command]
fn check_database_access() -> DatabaseStatus {
    let path = match get_imessage_db_path() {
        Some(p) => p,
        None => {
            return DatabaseStatus {
                accessible: false,
                path: String::new(),
                error: Some("Could not determine home directory".to_string()),
            }
        }
    };

    let path_str = path.to_string_lossy().to_string();

    // Try to open the database
    match Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => {
            // Try a simple query to verify we can actually read
            match conn.query_row("SELECT COUNT(*) FROM message", [], |row| row.get::<_, i64>(0)) {
                Ok(_) => DatabaseStatus {
                    accessible: true,
                    path: path_str,
                    error: None,
                },
                Err(e) => DatabaseStatus {
                    accessible: false,
                    path: path_str,
                    error: Some(format!("Cannot read database: {}", e)),
                },
            }
        }
        Err(e) => DatabaseStatus {
            accessible: false,
            path: path_str,
            error: Some(format!("Cannot open database. Please grant Full Disk Access in System Settings > Privacy & Security > Full Disk Access. Error: {}", e)),
        },
    }
}

/// Get all contacts with message counts
#[tauri::command]
fn get_contacts() -> Result<Vec<Contact>, String> {
    let path = get_imessage_db_path().ok_or("Could not find iMessage database")?;
    let conn = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("Cannot open database: {}", e))?;

    let mut stmt = conn
        .prepare(
            "SELECT h.ROWID, h.id, h.uncanonicalized_id, COUNT(m.ROWID) as msg_count
             FROM handle h
             LEFT JOIN message m ON m.handle_id = h.ROWID
             GROUP BY h.ROWID
             ORDER BY msg_count DESC",
        )
        .map_err(|e| format!("Query error: {}", e))?;

    let contacts = stmt
        .query_map([], |row| {
            Ok(Contact {
                id: row.get(0)?,
                identifier: row.get::<_, String>(1)?,
                display_name: row.get::<_, Option<String>>(2).ok().flatten(),
                message_count: row.get(3)?,
            })
        })
        .map_err(|e| format!("Query error: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(contacts)
}

/// Get chat statistics
#[tauri::command]
fn get_chat_stats(options: Option<ExportOptions>) -> Result<ChatStats, String> {
    let path = get_imessage_db_path().ok_or("Could not find iMessage database")?;
    let conn = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("Cannot open database: {}", e))?;

    let mut where_clauses = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref opts) = options {
        if let Some(start) = opts.start_date {
            let mac_start = (start - MAC_EPOCH_OFFSET) * 1_000_000_000;
            where_clauses.push("date >= ?");
            params.push(Box::new(mac_start));
        }
        if let Some(end) = opts.end_date {
            let mac_end = (end - MAC_EPOCH_OFFSET) * 1_000_000_000;
            where_clauses.push("date <= ?");
            params.push(Box::new(mac_end));
        }
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Total messages
    let total_messages: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM message {}", where_sql),
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )
        .map_err(|e| format!("Query error: {}", e))?;

    // Messages sent
    let mut params2: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(ref opts) = options {
        if let Some(start) = opts.start_date {
            let mac_start = (start - MAC_EPOCH_OFFSET) * 1_000_000_000;
            params2.push(Box::new(mac_start));
        }
        if let Some(end) = opts.end_date {
            let mac_end = (end - MAC_EPOCH_OFFSET) * 1_000_000_000;
            params2.push(Box::new(mac_end));
        }
    }

    let sent_where = if where_clauses.is_empty() {
        "WHERE is_from_me = 1".to_string()
    } else {
        format!("{} AND is_from_me = 1", where_sql)
    };

    let messages_sent: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM message {}", sent_where),
            rusqlite::params_from_iter(params2.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )
        .map_err(|e| format!("Query error: {}", e))?;

    // Total contacts
    let total_contacts: i64 = conn
        .query_row("SELECT COUNT(*) FROM handle", [], |row| row.get(0))
        .map_err(|e| format!("Query error: {}", e))?;

    // Date range
    let (date_start, date_end): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT MIN(date), MAX(date) FROM message WHERE date > 0",
            [],
            |row| {
                let min: Option<i64> = row.get(0).ok();
                let max: Option<i64> = row.get(1).ok();
                Ok((
                    min.map(mac_timestamp_to_unix),
                    max.map(mac_timestamp_to_unix),
                ))
            },
        )
        .map_err(|e| format!("Query error: {}", e))?;

    Ok(ChatStats {
        total_messages,
        messages_sent,
        messages_received: total_messages - messages_sent,
        total_contacts,
        date_range_start: date_start,
        date_range_end: date_end,
    })
}

/// Get messages with optional filtering
#[tauri::command]
fn get_messages(options: Option<ExportOptions>, limit: Option<i64>) -> Result<Vec<Message>, String> {
    let path = get_imessage_db_path().ok_or("Could not find iMessage database")?;
    let conn = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("Cannot open database: {}", e))?;

    let mut where_clauses = vec!["m.date > 0".to_string()];
    let mut params: Vec<i64> = Vec::new();

    if let Some(ref opts) = options {
        if let Some(start) = opts.start_date {
            let mac_start = (start - MAC_EPOCH_OFFSET) * 1_000_000_000;
            where_clauses.push("m.date >= ?".to_string());
            params.push(mac_start);
        }
        if let Some(end) = opts.end_date {
            let mac_end = (end - MAC_EPOCH_OFFSET) * 1_000_000_000;
            where_clauses.push("m.date <= ?".to_string());
            params.push(mac_end);
        }
        if let Some(ref contact_ids) = opts.contact_ids {
            if !contact_ids.is_empty() {
                let placeholders: Vec<String> = contact_ids.iter().map(|_| "?".to_string()).collect();
                where_clauses.push(format!("m.handle_id IN ({})", placeholders.join(",")));
                params.extend(contact_ids.iter().cloned());
            }
        }
    }

    let where_sql = where_clauses.join(" AND ");
    let limit_sql = limit.map(|l| format!("LIMIT {}", l)).unwrap_or_default();

    let query = format!(
        "SELECT m.ROWID, m.text, m.date, m.is_from_me, m.handle_id,
                COALESCE(h.id, 'Unknown') as contact_id,
                m.cache_has_attachments,
                cmj.chat_id
         FROM message m
         LEFT JOIN handle h ON m.handle_id = h.ROWID
         LEFT JOIN chat_message_join cmj ON m.ROWID = cmj.message_id
         WHERE {}
         ORDER BY m.date DESC
         {}",
        where_sql, limit_sql
    );

    let mut stmt = conn.prepare(&query).map_err(|e| format!("Query error: {}", e))?;

    let messages = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |row| {
            let mac_date: i64 = row.get(2)?;
            let unix_date = mac_timestamp_to_unix(mac_date);
            let datetime = Utc.timestamp_opt(unix_date, 0).single();
            let date_formatted = datetime
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            Ok(Message {
                id: row.get(0)?,
                text: row.get(1)?,
                date: unix_date,
                date_formatted,
                is_from_me: row.get::<_, i64>(3)? == 1,
                handle_id: row.get(4)?,
                contact_identifier: row.get(5)?,
                chat_id: row.get(7)?,
                has_attachment: row.get::<_, i64>(6)? == 1,
            })
        })
        .map_err(|e| format!("Query error: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(messages)
}

/// Get messages for a specific contact formatted for export
#[tauri::command]
fn get_messages_for_contact(contact_id: i64, options: Option<ExportOptions>) -> Result<Vec<Message>, String> {
    let mut opts = options.unwrap_or(ExportOptions {
        start_date: None,
        end_date: None,
        contact_ids: None,
    });
    opts.contact_ids = Some(vec![contact_id]);
    get_messages(Some(opts), None)
}

/// Open System Preferences to Full Disk Access
#[tauri::command]
fn open_system_preferences() -> Result<(), String> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
        .spawn()
        .map_err(|e| format!("Failed to open System Preferences: {}", e))?;
    Ok(())
}

/// Open System Preferences to Contacts
#[tauri::command]
fn open_contacts_preferences() -> Result<(), String> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Contacts")
        .spawn()
        .map_err(|e| format!("Failed to open System Preferences: {}", e))?;
    Ok(())
}

/// Check if we can access the Contacts database
#[tauri::command]
fn check_contacts_access() -> bool {
    let contact_names = get_contact_names();
    !contact_names.is_empty()
}

/// Get all chats with participants and message counts
#[tauri::command]
fn get_chats() -> Result<Vec<Chat>, String> {
    let path = get_imessage_db_path().ok_or("Could not find iMessage database")?;
    let conn = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("Cannot open database: {}", e))?;

    // Load contact names for resolution
    let contact_names = get_contact_names();

    // Get all chats with message counts
    let mut stmt = conn
        .prepare(
            "SELECT c.ROWID, c.chat_identifier, c.display_name, c.style,
                    COUNT(DISTINCT cmj.message_id) as msg_count
             FROM chat c
             LEFT JOIN chat_message_join cmj ON c.ROWID = cmj.chat_id
             GROUP BY c.ROWID
             ORDER BY msg_count DESC",
        )
        .map_err(|e| format!("Query error: {}", e))?;

    let mut chats: Vec<Chat> = stmt
        .query_map([], |row| {
            let style: i64 = row.get(3)?;
            Ok(Chat {
                id: row.get(0)?,
                chat_identifier: row.get(1)?,
                display_name: row.get::<_, Option<String>>(2).ok().flatten(),
                is_group: style == 43, // 43 = group chat, 45 = individual
                participant_count: 0,
                message_count: row.get(4)?,
                participants: Vec::new(),
            })
        })
        .map_err(|e| format!("Query error: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    // Get participants for each chat and resolve names
    for chat in &mut chats {
        let mut participant_stmt = conn
            .prepare(
                "SELECT h.id FROM handle h
                 JOIN chat_handle_join chj ON h.ROWID = chj.handle_id
                 WHERE chj.chat_id = ?",
            )
            .map_err(|e| format!("Query error: {}", e))?;

        let raw_participants: Vec<String> = participant_stmt
            .query_map([chat.id], |row| row.get(0))
            .map_err(|e| format!("Query error: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        // Resolve participant names
        let participants: Vec<String> = raw_participants
            .iter()
            .map(|p| {
                lookup_contact_name(p, &contact_names)
                    .unwrap_or_else(|| p.clone())
            })
            .collect();

        chat.participant_count = participants.len() as i64;
        chat.participants = participants;

        // For individual chats without display_name, try to set it from contact
        if chat.display_name.is_none() && raw_participants.len() == 1 {
            if let Some(name) = lookup_contact_name(&raw_participants[0], &contact_names) {
                chat.display_name = Some(name);
            }
        }
    }

    Ok(chats)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_database_access,
            check_contacts_access,
            get_contacts,
            get_chats,
            get_chat_stats,
            get_messages,
            get_messages_for_contact,
            open_system_preferences,
            open_contacts_preferences,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
