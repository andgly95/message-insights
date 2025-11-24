# iMessage Insights

A powerful, privacy-first iMessage analysis and visualization tool. View, search, and analyze your iMessage conversations with beautiful visualizations and deep analytics.

## ✨ Features

### Current Features

- **📱 Beautiful Viewer Interface**
  - Dark mode UI inspired by macOS Messages
  - Conversation list with search
  - Message bubbles with timestamps, reactions, and read receipts
  - Image and attachment support

- **📊 Deep Conversation Analysis**
  - **Word Frequency Analysis** - Most used words with visual bars
  - **N-gram Analysis** - Common 2-word and 3-word phrases
  - **Sentiment Analysis** - Positive/neutral/negative message classification
  - **Time Patterns** - Activity heatmaps by hour and day of week
  - **Emoji Analytics** - Most used emojis with counts
  - **Response Time Analysis** - Average response times between participants
  - **Participant Stats** - Per-person message counts, average length, questions asked
  - **Link Analysis** - Most shared domains and URLs
  - **Export Functionality** - Download analysis data as JSON

- **🔍 Search & Filter**
  - Real-time word search across analysis data
  - Conversation search in sidebar
  - Filter by participants

## 🚀 Getting Started

### Current Usage

1. Place your exported `.txt` message files in the `messages/` folder
2. Run the generator script:
   ```bash
   python3 generate_viewer.py
   ```
3. Open `viewer.html` in your browser to view your messages

### File Structure

```
imessage-insights/
├── generate_viewer.py      # Script to embed message data into viewer
├── viewer_template.html    # HTML template with all features
├── viewer.html            # Generated viewer (not committed)
├── messages/              # Your message data (not committed)
│   ├── *.txt             # Individual conversation exports
│   └── attachments/      # Message attachments
└── README.md
```

## 🔮 Roadmap

### 🚧 Next Up: Direct iMessage Import

Integration with [imessage-exporter](https://github.com/ReagentX/imessage-exporter) for seamless message import:

- **Welcome Flow** - First-time user onboarding when no messages are loaded
- **Date Range Selection** - Choose which messages to import
- **Export Options** - Configure export settings (contacts, attachments, etc.)
- **Live Progress** - Real-time progress indicators during export
- **Incremental Updates** - Add new messages without re-exporting everything
- **Direct Integration** - No manual file management needed

### Future Ideas

- 📈 Timeline visualizations for word usage over time
- 🔗 Conversation comparison features
- 📝 Custom analysis rules and filters
- 🌐 Multi-platform support (WhatsApp, Signal, etc.)
- 📊 Interactive charts with drill-down capabilities

## 🔒 Privacy

Your messages never leave your computer. This is a completely local, client-side tool:

- No servers, no cloud, no tracking
- All processing happens in your browser
- Message data is excluded from git via `.gitignore`
- Generated viewer file is self-contained and portable

## 🛠️ Technical Details

- **Frontend**: Pure HTML/CSS/JavaScript (no frameworks)
- **Backend**: Python 3 for data processing
- **Storage**: Flat text files (compatible with imessage-exporter format)
- **Size**: ~50KB template expands to ~10MB with embedded data

## 📊 Analysis Features Deep Dive

### Word Frequency
Uses a hashmap for O(1) word counting with stop-word filtering. Displays top 30 words with visual frequency bars.

### N-grams
Sliding window approach to extract common 2-word and 3-word phrases, sorted by frequency.

### Sentiment Analysis
Dictionary-based classification using positive/negative word sets. Tracks sentiment over time by month.

### Time Patterns
- **Hour Heatmap**: 24-hour activity visualization with gradient intensity
- **Day of Week Chart**: Vertical bar chart showing message distribution across days
- **Monthly Trends**: Message counts aggregated by month

### Response Time Analysis
Calculates time between messages from different participants (max 24-hour window to filter out async conversations).

## 🤝 Contributing

This is a personal project, but suggestions and ideas are welcome! Open an issue or reach out.

## 📄 License

MIT License - feel free to use and modify for your own purposes.

## 🙏 Acknowledgments

- Message export format compatible with [imessage-exporter](https://github.com/ReagentX/imessage-exporter)
- UI inspired by macOS Messages app
- Built with privacy and simplicity in mind
