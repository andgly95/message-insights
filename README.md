# Message Insights

A powerful, privacy-first native macOS app for viewing and analyzing your iMessage conversations. All data stays on your device.

## Features

### Message Viewer
- Dark mode UI inspired by macOS Messages
- Conversation list with search
- Message bubbles with timestamps, reactions, and read receipts
- Image and video attachment display
- Virtual scrolling for smooth performance with large conversations
- Contact name resolution from your address book

### Deep Conversation Analysis
- **Word Frequency Analysis** - Most used words with visual bars
- **N-gram Analysis** - Common 2-word and 3-word phrases
- **Sentiment Analysis** - Positive/neutral/negative message classification
- **Time Patterns** - Activity heatmaps by hour and day of week
- **Emoji Analytics** - Most used emojis with counts
- **Response Time Analysis** - Average response times between participants
- **Participant Stats** - Per-person message counts, average length, questions asked
- **Link Analysis** - Most shared domains and URLs
- **Export Functionality** - Download analysis data as JSON

### Search & Filter
- Real-time word search across analysis data
- Conversation search in sidebar
- Date range filtering
- Contact filtering

## Installation

### Prerequisites
- macOS 10.15 or later
- Full Disk Access permission (to read iMessage database)
- Contacts access (optional, for contact name resolution)

### Building from Source

1. Install Rust and Node.js
2. Clone the repository:
   ```bash
   git clone https://github.com/andgly95/message-insights.git
   cd message-insights
   ```
3. Install dependencies:
   ```bash
   npm install
   ```
4. Build the app:
   ```bash
   npm run tauri build
   ```
5. The built app will be in `src-tauri/target/release/bundle/`

### Development

Run in development mode:
```bash
npm run tauri dev
```

## Permissions

Message Insights requires the following permissions:

### Full Disk Access (Required)
To read your iMessage database at `~/Library/Messages/chat.db`:
1. Open **System Settings** > **Privacy & Security** > **Full Disk Access**
2. Click **+** and add **Message Insights**
3. Restart the app

### Contacts Access (Optional)
To display contact names instead of phone numbers:
1. Open **System Settings** > **Privacy & Security** > **Contacts**
2. Enable access for **Message Insights**

## Privacy

Your messages never leave your computer:
- No servers, no cloud, no tracking
- All processing happens locally on your device
- Direct read-only access to your iMessage database
- No data is collected or transmitted

## Technical Details

- **Frontend**: HTML/CSS/JavaScript with Tauri WebView
- **Backend**: Rust with SQLite for database access
- **Framework**: Tauri v2
- **Database**: Direct read-only access to macOS iMessage SQLite database

### Project Structure

```
message-insights/
├── src/                    # Frontend (HTML/CSS/JS)
│   └── index.html         # Main application UI
├── src-tauri/             # Rust backend
│   ├── src/
│   │   └── lib.rs         # Tauri commands and database access
│   ├── Cargo.toml         # Rust dependencies
│   └── tauri.conf.json    # Tauri configuration
└── package.json           # Node.js dependencies
```

## License

MIT License - feel free to use and modify for your own purposes.

## Acknowledgments

- UI inspired by macOS Messages app
- Built with [Tauri](https://tauri.app/) for native performance
- Uses [rusqlite](https://github.com/rusqlite/rusqlite) for SQLite access
