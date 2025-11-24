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

/// Get ALL paths to AddressBook databases (iCloud, local, Exchange, etc.)
fn get_all_addressbook_db_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return paths,
    };

    let sources_dir = home.join("Library/Application Support/AddressBook/Sources");

    // Find ALL source directories with AddressBook databases
    if let Ok(entries) = std::fs::read_dir(&sources_dir) {
        for entry in entries.flatten() {
            let db_path = entry.path().join("AddressBook-v22.abcddb");
            if db_path.exists() {
                paths.push(db_path);
            }
        }
    }

    // Also check direct path (older macOS versions)
    let direct_path = home.join("Library/Application Support/AddressBook/AddressBook-v22.abcddb");
    if direct_path.exists() {
        paths.push(direct_path);
    }

    paths
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

/// Read contacts from a single AddressBook database
fn read_contacts_from_db(db_path: &PathBuf, names: &mut HashMap<String, String>) {
    let conn = match Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(c) => c,
        Err(_) => return,
    };

    // Query for phone numbers
    let phone_results: Vec<(Option<String>, Option<String>, String)> = {
        let phone_query = "
            SELECT ZABCDRECORD.ZFIRSTNAME, ZABCDRECORD.ZLASTNAME, ZABCDPHONENUMBER.ZFULLNUMBER
            FROM ZABCDRECORD
            LEFT JOIN ZABCDPHONENUMBER ON ZABCDRECORD.Z_PK = ZABCDPHONENUMBER.ZOWNER
            WHERE ZABCDPHONENUMBER.ZFULLNUMBER IS NOT NULL
        ";
        conn.prepare(phone_query)
            .ok()
            .map(|mut stmt| {
                stmt.query_map([], |row| {
                    let first: Option<String> = row.get(0).ok();
                    let last: Option<String> = row.get(1).ok();
                    let phone: String = row.get(2)?;
                    Ok((first, last, phone))
                })
                .map(|rows| rows.flatten().collect())
                .unwrap_or_default()
            })
            .unwrap_or_default()
    };

    for (first, last, phone) in phone_results {
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

    // Query for email addresses
    let email_results: Vec<(Option<String>, Option<String>, String)> = {
        let email_query = "
            SELECT ZABCDRECORD.ZFIRSTNAME, ZABCDRECORD.ZLASTNAME, ZABCDEMAILADDRESS.ZADDRESS
            FROM ZABCDRECORD
            LEFT JOIN ZABCDEMAILADDRESS ON ZABCDRECORD.Z_PK = ZABCDEMAILADDRESS.ZOWNER
            WHERE ZABCDEMAILADDRESS.ZADDRESS IS NOT NULL
        ";
        conn.prepare(email_query)
            .ok()
            .map(|mut stmt| {
                stmt.query_map([], |row| {
                    let first: Option<String> = row.get(0).ok();
                    let last: Option<String> = row.get(1).ok();
                    let email: String = row.get(2)?;
                    Ok((first, last, email))
                })
                .map(|rows| rows.flatten().collect())
                .unwrap_or_default()
            })
            .unwrap_or_default()
    };

    for (first, last, email) in email_results {
        let name = match (first, last) {
            (Some(f), Some(l)) => format!("{} {}", f, l),
            (Some(f), None) => f,
            (None, Some(l)) => l,
            (None, None) => continue,
        };
        names.insert(email.to_lowercase(), name);
    }
}

/// Get contact name mappings from ALL AddressBook databases
fn get_contact_names() -> HashMap<String, String> {
    let mut names: HashMap<String, String> = HashMap::new();

    let db_paths = get_all_addressbook_db_paths();

    // Read from ALL AddressBook databases (iCloud, local, Exchange, etc.)
    for db_path in &db_paths {
        read_contacts_from_db(db_path, &mut names);
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
    pub participants: Vec<String>,          // Resolved names
    pub participant_ids: Vec<String>,       // Raw phone/email identifiers
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: i64,
    pub guid: String,
    pub text: Option<String>,
    pub date: i64,               // Unix timestamp
    pub date_formatted: String,
    pub is_from_me: bool,
    pub handle_id: i64,
    pub contact_identifier: String,
    pub sender_name: String,     // Resolved sender name
    pub chat_id: Option<i64>,
    pub has_attachment: bool,
    pub attachments: Vec<Attachment>,
    pub reactions: Vec<Reaction>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Attachment {
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub transfer_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Reaction {
    pub reaction_type: i64,   // 2000=love, 2001=like, 2002=dislike, 2003=laugh, 2004=emphasis, 2005=question
    pub sender: String,
    pub is_from_me: bool,
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

    // Load contact names for reaction sender resolution
    let contact_names = get_contact_names();

    let mut where_clauses = vec![
        "m.date > 0".to_string(),
        // Exclude reaction messages (associated_message_type >= 2000) and edit messages (1000-1999)
        "(m.associated_message_type IS NULL OR m.associated_message_type = 0)".to_string(),
    ];
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
        "SELECT m.ROWID, m.guid, m.text, m.date, m.is_from_me, COALESCE(m.handle_id, 0),
                COALESCE(h.id, '') as contact_id,
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

    let mut messages: Vec<Message> = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |row| {
            let mac_date: i64 = row.get(3)?;
            let unix_date = mac_timestamp_to_unix(mac_date);
            let datetime = Utc.timestamp_opt(unix_date, 0).single();
            let date_formatted = datetime
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            let is_from_me = row.get::<_, i64>(4)? == 1;
            let contact_identifier: String = row.get(6)?;
            let text: Option<String> = row.get(2)?;

            // Resolve sender name
            let sender_name = if is_from_me {
                "Me".to_string()
            } else if contact_identifier.is_empty() {
                "Unknown".to_string()
            } else {
                // Will be resolved after query
                contact_identifier.clone()
            };

            Ok(Message {
                id: row.get(0)?,
                guid: row.get(1)?,
                text,
                date: unix_date,
                date_formatted,
                is_from_me,
                handle_id: row.get(5)?,
                contact_identifier,
                sender_name,
                chat_id: row.get(8)?,
                has_attachment: row.get::<_, i64>(7)? == 1,
                attachments: Vec::new(),
                reactions: Vec::new(),
            })
        })
        .map_err(|e| format!("Query error: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    // Resolve sender names from contacts
    for msg in &mut messages {
        if !msg.is_from_me && !msg.contact_identifier.is_empty() {
            if let Some(name) = lookup_contact_name(&msg.contact_identifier, &contact_names) {
                msg.sender_name = name;
            }
        }
    }

    // Build a GUID lookup for attaching reactions
    let guid_to_idx: HashMap<String, usize> = messages.iter().enumerate()
        .map(|(i, m)| (m.guid.clone(), i))
        .collect();

    // Fetch attachments for messages that have them
    let message_ids: Vec<i64> = messages.iter()
        .filter(|m| m.has_attachment)
        .map(|m| m.id)
        .collect();

    if !message_ids.is_empty() {
        let placeholders: String = message_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let attach_query = format!(
            "SELECT maj.message_id, a.filename, a.mime_type, a.transfer_name
             FROM message_attachment_join maj
             JOIN attachment a ON maj.attachment_id = a.ROWID
             WHERE maj.message_id IN ({})",
            placeholders
        );

        if let Ok(mut attach_stmt) = conn.prepare(&attach_query) {
            if let Ok(rows) = attach_stmt.query_map(rusqlite::params_from_iter(message_ids.iter()), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            }) {
                for row in rows.flatten() {
                    let (msg_id, filename, mime_type, transfer_name) = row;
                    if let Some(msg) = messages.iter_mut().find(|m| m.id == msg_id) {
                        msg.attachments.push(Attachment {
                            filename,
                            mime_type,
                            transfer_name,
                        });
                    }
                }
            }
        }
    }

    // Fetch reactions for all messages
    // Reactions have associated_message_type between 2000-2005 and reference parent via associated_message_guid
    let reaction_query = "
        SELECT m.associated_message_guid, m.associated_message_type, m.is_from_me, COALESCE(h.id, '') as sender
        FROM message m
        LEFT JOIN handle h ON m.handle_id = h.ROWID
        WHERE m.associated_message_type >= 2000 AND m.associated_message_type < 3000
    ";

    if let Ok(mut reaction_stmt) = conn.prepare(reaction_query) {
        if let Ok(rows) = reaction_stmt.query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)? == 1,
                row.get::<_, String>(3)?,
            ))
        }) {
            for row in rows.flatten() {
                let (assoc_guid_opt, reaction_type, is_from_me, sender_id) = row;
                if let Some(assoc_guid) = assoc_guid_opt {
                    // The associated_message_guid has format like "p:0/guid" or "bp:guid"
                    // Extract the actual GUID part
                    let clean_guid = assoc_guid
                        .split('/')
                        .last()
                        .unwrap_or(&assoc_guid)
                        .to_string();

                    if let Some(&idx) = guid_to_idx.get(&clean_guid) {
                        let sender = if is_from_me {
                            "Me".to_string()
                        } else {
                            lookup_contact_name(&sender_id, &contact_names)
                                .unwrap_or_else(|| sender_id.clone())
                        };
                        messages[idx].reactions.push(Reaction {
                            reaction_type,
                            sender,
                            is_from_me,
                        });
                    }
                }
            }
        }
    }

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
                participant_ids: Vec::new(),
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
        chat.participant_ids = raw_participants.clone();

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
