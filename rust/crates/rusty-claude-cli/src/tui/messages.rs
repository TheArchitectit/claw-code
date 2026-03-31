//! Message rendering widgets for the TUI.
//!
//! This module provides widgets for rendering different message types
//! with styling, syntax highlighting, and markdown support.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, StatefulWidget, Widget, Wrap},
};

use runtime::messages::{
    AssistantMessage, ContentBlock, Message, MessageLevel, ProgressData, SystemMessage,
    ToolResultContent, ToolUseSummaryMessage, UserMessage,
};

/// A widget for rendering a message in the UI.
#[derive(Debug, Clone)]
pub enum MessageWidget {
    /// A user message widget.
    User(UserMessageWidget),
    /// An assistant message widget.
    Assistant(AssistantMessageWidget),
    /// A system message widget.
    System(SystemMessageWidget),
    /// A tool call widget (shows tool invocation).
    ToolCall(ToolCallWidget),
    /// A tool result widget (shows tool output).
    ToolResult(ToolResultWidget),
    /// A progress message widget.
    Progress(ProgressMessageWidget),
}

impl MessageWidget {
    /// Create a widget from a runtime message.
    pub fn from_message(message: &Message) -> Option<Self> {
        match message {
            Message::User(msg) => Some(Self::User(UserMessageWidget::from_message(msg))),
            Message::Assistant(msg) => Some(Self::Assistant(AssistantMessageWidget::from_message(msg))),
            Message::System(msg) => {
                SystemMessageWidget::from_message(msg).map(Self::System)
            }
            Message::Progress(msg) => Some(Self::Progress(ProgressMessageWidget::from_message(msg))),
            Message::ToolUseSummary(msg) => Some(Self::ToolResult(ToolResultWidget::from_summary(msg))),
            _ => None,
        }
    }

    /// Get the height of this widget for the given width.
    pub fn height(&self, width: u16) -> u16 {
        match self {
            Self::User(w) => w.height(width),
            Self::Assistant(w) => w.height(width),
            Self::System(w) => w.height(width),
            Self::ToolCall(w) => w.height(width),
            Self::ToolResult(w) => w.height(width),
            Self::Progress(w) => w.height(width),
        }
    }
}

impl Widget for MessageWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self {
            Self::User(w) => w.render(area, buf),
            Self::Assistant(w) => w.render(area, buf),
            Self::System(w) => w.render(area, buf),
            Self::ToolCall(w) => w.render(area, buf),
            Self::ToolResult(w) => w.render(area, buf),
            Self::Progress(w) => w.render(area, buf),
        }
    }
}

/// Widget for rendering user messages.
#[derive(Debug, Clone)]
pub struct UserMessageWidget {
    /// The message content.
    pub content: String,
    /// Whether to show a bubble.
    pub show_bubble: bool,
}

impl UserMessageWidget {
    /// Create a new user message widget.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            show_bubble: true,
        }
    }

    /// Create from a runtime UserMessage.
    #[must_use]
    pub fn from_message(message: &UserMessage) -> Self {
        let content = message
            .content
            .content
            .iter()
            .map(|block| match block {
                ContentBlock::Text { text, .. } => text.as_str(),
                _ => "",
            })
            .collect::<Vec<_>>()
            .concat();
        Self::new(content)
    }

    /// Set whether to show a bubble.
    #[must_use]
    pub fn with_bubble(mut self, show: bool) -> Self {
        self.show_bubble = show;
        self
    }

    /// Calculate height for the given width.
    #[must_use]
    pub fn height(&self, width: u16) -> u16 {
        let text_width = if self.show_bubble {
            width.saturating_sub(4) // Padding for bubble
        } else {
            width
        };
        let lines = textwrap::wrap(&self.content, text_width as usize);
        lines.len() as u16 + if self.show_bubble { 2 } else { 0 }
    }
}

impl Widget for UserMessageWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let style = Style::default().fg(Color::Blue);
        let prefix = Span::styled("You: ", style.add_modifier(Modifier::BOLD));

        if self.show_bubble {
            // Render with bubble
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .style(Style::default().bg(Color::Black));
            let inner = block.inner(area);
            block.render(area, buf);

            let text = Text::from(self.content.clone());
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .render(inner, buf);
        } else {
            // Render without bubble
            let lines = vec![
                Line::from(prefix),
                Line::from(self.content.clone()),
            ];
            let text = Text::from(lines);
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .render(area, buf);
        }
    }
}

/// Widget for rendering assistant messages.
#[derive(Debug, Clone)]
pub struct AssistantMessageWidget {
    /// The message content blocks.
    pub content: Vec<ContentBlock>,
    /// The model that generated this message.
    pub model: Option<String>,
    /// Whether to show thinking blocks.
    pub show_thinking: bool,
    /// Whether the message is streaming.
    pub is_streaming: bool,
}

impl AssistantMessageWidget {
    /// Create a new assistant message widget.
    #[must_use]
    pub fn new(content: Vec<ContentBlock>) -> Self {
        Self {
            content,
            model: None,
            show_thinking: false,
            is_streaming: false,
        }
    }

    /// Create from a runtime AssistantMessage.
    #[must_use]
    pub fn from_message(message: &AssistantMessage) -> Self {
        Self {
            content: message.content.content.clone(),
            model: Some(message.content.model.clone()),
            show_thinking: false,
            is_streaming: false,
        }
    }

    /// Set whether to show thinking blocks.
    #[must_use]
    pub fn with_thinking(mut self, show: bool) -> Self {
        self.show_thinking = show;
        self
    }

    /// Set streaming state.
    #[must_use]
    pub fn with_streaming(mut self, streaming: bool) -> Self {
        self.is_streaming = streaming;
        self
    }

    /// Calculate height for the given width.
    #[must_use]
    pub fn height(&self, width: u16) -> u16 {
        let mut height = 2; // Header + spacing

        for block in &self.content {
            match block {
                ContentBlock::Text { text, .. } => {
                    let lines = textwrap::wrap(text, width as usize);
                    height += lines.len() as u16;
                }
                ContentBlock::Thinking { thinking, .. } => {
                    if self.show_thinking {
                        let lines = textwrap::wrap(thinking, width.saturating_sub(4) as usize);
                        height += lines.len() as u16 + 2;
                    }
                }
                ContentBlock::ToolUse { name, .. } => {
                    height += 2; // Tool use header
                    height += textwrap::wrap(name, width as usize).len() as u16;
                }
                _ => height += 1,
            }
        }

        height
    }

    /// Render content blocks to lines.
    fn render_content(&self, width: u16) -> Vec<Line<'_>> {
        let mut lines = Vec::new();

        for block in &self.content {
            match block {
                ContentBlock::Text { text, .. } => {
                    let wrapped = textwrap::wrap(text, width as usize);
                    for line in wrapped {
                        lines.push(Line::from(line.to_string()));
                    }
                    lines.push(Line::from(""));
                }
                ContentBlock::Thinking { thinking, .. } => {
                    if self.show_thinking {
                        lines.push(Line::from(Span::styled(
                            "🤔 Thinking...",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::ITALIC),
                        )));
                        let wrapped = textwrap::wrap(thinking, width.saturating_sub(4) as usize);
                        for line in wrapped {
                            lines.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled(
                                    line.to_string(),
                                    Style::default().fg(Color::Yellow),
                                ),
                            ]));
                        }
                        lines.push(Line::from(""));
                    }
                }
                ContentBlock::ToolUse { id, name, input } => {
                    let tool_style = Style::default().fg(Color::Magenta);
                    let id_str = format!("{:?}", id);
                    lines.push(Line::from(vec![
                        Span::styled("🔧 Tool: ", tool_style),
                        Span::styled(id_str, tool_style.add_modifier(Modifier::BOLD)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("   Name: ", tool_style),
                        Span::raw(name.clone()),
                    ]));
                    if let Ok(json_str) = serde_json::to_string_pretty(input) {
                        for json_line in json_str.lines() {
                            lines.push(Line::from(vec![
                                Span::raw("   "),
                                Span::styled(json_line.to_string(), Style::default().fg(Color::Cyan)),
                            ]));
                        }
                    }
                    lines.push(Line::from(""));
                }
                _ => {}
            }
        }

        lines
    }
}

impl Widget for AssistantMessageWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let header_style = Style::default().fg(Color::Green).add_modifier(Modifier::BOLD);

        // Header with model info
        let header = if let Some(model) = &self.model {
            if self.is_streaming {
                Line::from(vec![
                    Span::styled("Claude (", header_style),
                    Span::styled(model.clone(), header_style),
                    Span::styled(") ▌", header_style),
                ])
            } else {
                Line::from(vec![
                    Span::styled("Claude (", header_style),
                    Span::styled(model.clone(), header_style),
                    Span::styled(")", header_style),
                ])
            }
        } else {
            Line::from(Span::styled("Claude", header_style))
        };

        let content_lines = self.render_content(area.width);
        let mut all_lines = vec![header, Line::from("")];
        all_lines.extend(content_lines);

        let text = Text::from(all_lines);
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

/// Widget for rendering system messages.
#[derive(Debug, Clone)]
pub struct SystemMessageWidget {
    /// The message content.
    pub content: String,
    /// The message level.
    pub level: MessageLevel,
}

impl SystemMessageWidget {
    /// Create a new system message widget.
    #[must_use]
    pub fn new(content: impl Into<String>, level: MessageLevel) -> Self {
        Self {
            content: content.into(),
            level,
        }
    }

    /// Create from a runtime SystemMessage.
    #[must_use]
    pub fn from_message(message: &SystemMessage) -> Option<Self> {
        use runtime::messages::SystemMessageSubtype;
        match &message.subtype {
            SystemMessageSubtype::Informational { level, content } => {
                Some(Self::new(content.clone(), *level))
            }
            SystemMessageSubtype::ApiError { error, retry_attempt, max_retries, .. } => {
                let content = format!(
                    "API Error{}: {} (attempt {}/{})",
                    error.error_type.as_ref().map(|t| format!(" ({})", t)).unwrap_or_default(),
                    error.message,
                    retry_attempt,
                    max_retries
                );
                Some(Self::new(content, MessageLevel::Error))
            }
            SystemMessageSubtype::LocalCommand { content } => {
                Some(Self::new(content.clone(), MessageLevel::Info))
            }
            _ => None, // Other subtypes don't have simple content/level display
        }
    }

    /// Get the color for this message level.
    #[must_use]
    pub fn level_color(&self) -> Color {
        match self.level {
            MessageLevel::Info => Color::Blue,
            MessageLevel::Warn => Color::Yellow,
            MessageLevel::Error => Color::Red,
        }
    }

    /// Get the prefix icon for this level.
    #[must_use]
    pub fn level_icon(&self) -> &'static str {
        match self.level {
            MessageLevel::Info => "ℹ",
            MessageLevel::Warn => "⚠",
            MessageLevel::Error => "✖",
        }
    }

    /// Calculate height for the given width.
    #[must_use]
    pub fn height(&self, width: u16) -> u16 {
        let lines = textwrap::wrap(&self.content, width as usize);
        lines.len() as u16 + 1
    }
}

impl Widget for SystemMessageWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let color = self.level_color();
        let icon = self.level_icon();

        let lines = vec![
            Line::from(vec![
                Span::styled(
                    format!("{} System: ", icon),
                    Style::default()
                        .fg(color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(self.content.clone(), Style::default().fg(color)),
            ]),
        ];

        let text = Text::from(lines);
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

/// Widget for rendering tool calls.
#[derive(Debug, Clone)]
pub struct ToolCallWidget {
    /// The tool name.
    pub tool_name: String,
    /// The tool use ID.
    pub tool_use_id: String,
    /// The tool input.
    pub input: serde_json::Value,
    /// Whether the tool is currently running.
    pub is_running: bool,
    /// Spinner frame for animation.
    pub spinner_frame: usize,
}

impl ToolCallWidget {
    /// Create a new tool call widget.
    #[must_use]
    pub fn new(
        tool_name: impl Into<String>,
        tool_use_id: impl Into<String>,
        input: serde_json::Value,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            tool_use_id: tool_use_id.into(),
            input,
            is_running: true,
            spinner_frame: 0,
        }
    }

    /// Set running state.
    #[must_use]
    pub fn with_running(mut self, running: bool) -> Self {
        self.is_running = running;
        self
    }

    /// Set spinner frame.
    #[must_use]
    pub fn with_spinner_frame(mut self, frame: usize) -> Self {
        self.spinner_frame = frame;
        self
    }

    /// Get the current spinner character.
    #[must_use]
    pub fn spinner(&self) -> &'static str {
        const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        SPINNER[self.spinner_frame % SPINNER.len()]
    }

    /// Calculate height for the given width.
    #[must_use]
    pub fn height(&self, width: u16) -> u16 {
        let mut height = 3; // Header + spacing

        // Calculate input JSON height
        if let Ok(json_str) = serde_json::to_string_pretty(&self.input) {
            let json_lines = textwrap::wrap(&json_str, width.saturating_sub(4) as usize);
            height += json_lines.len() as u16;
        }

        height
    }
}

impl Widget for ToolCallWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let header_style = Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD);
        let spinner = if self.is_running {
            self.spinner()
        } else {
            "✓"
        };
        let spinner_color = if self.is_running {
            Color::Yellow
        } else {
            Color::Green
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled(format!("{} ", spinner), Style::default().fg(spinner_color)),
                Span::styled("Tool: ", header_style),
                Span::styled(self.tool_name.clone(), header_style),
            ]),
        ];

        // Render input as pretty JSON
        if let Ok(json_str) = serde_json::to_string_pretty(&self.input) {
            for line in json_str.lines() {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(line.to_string(), Style::default().fg(Color::Cyan)),
                ]));
            }
        }

        lines.push(Line::from(""));

        let text = Text::from(lines);
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

/// Widget for rendering tool results.
#[derive(Debug, Clone)]
pub struct ToolResultWidget {
    /// The tool use ID.
    pub tool_use_id: String,
    /// The result content.
    pub content: Vec<ToolResultContent>,
    /// Whether the result is an error.
    pub is_error: bool,
    /// Summary text (optional).
    pub summary: Option<String>,
}

impl ToolResultWidget {
    /// Create a new tool result widget.
    #[must_use]
    pub fn new(tool_use_id: impl Into<String>, content: Vec<ToolResultContent>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content,
            is_error: false,
            summary: None,
        }
    }

    /// Create from a ToolUseSummaryMessage.
    #[must_use]
    pub fn from_summary(message: &ToolUseSummaryMessage) -> Self {
        Self {
            tool_use_id: "summary".to_string(),
            content: vec![ToolResultContent::Text {
                text: message.summary.clone(),
            }],
            is_error: false,
            summary: Some(message.summary.clone()),
        }
    }

    /// Set error state.
    #[must_use]
    pub fn with_error(mut self, error: bool) -> Self {
        self.is_error = error;
        self
    }

    /// Calculate height for the given width.
    #[must_use]
    pub fn height(&self, width: u16) -> u16 {
        let mut height = 2; // Header + spacing

        for content in &self.content {
            match content {
                ToolResultContent::Text { text } => {
                    let lines = textwrap::wrap(text, width.saturating_sub(2) as usize);
                    height += lines.len() as u16;
                }
                ToolResultContent::Json { data } => {
                    if let Ok(json_str) = serde_json::to_string_pretty(data) {
                        let json_lines = textwrap::wrap(&json_str, width.saturating_sub(4) as usize);
                        height += json_lines.len() as u16 + 2;
                    }
                }
                ToolResultContent::Error { message } => {
                    let lines = textwrap::wrap(message, width.saturating_sub(2) as usize);
                    height += lines.len() as u16;
                }
                _ => height += 1,
            }
        }

        height
    }

    /// Render content blocks to lines.
    fn render_content(&self, width: u16) -> Vec<Line<'_>> {
        let mut lines = Vec::new();

        for content in &self.content {
            match content {
                ToolResultContent::Text { text } => {
                    let wrapped = textwrap::wrap(text, width.saturating_sub(2) as usize);
                    for line in wrapped {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::raw(line.to_string()),
                        ]));
                    }
                }
                ToolResultContent::Json { data } => {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled("{", Style::default().fg(Color::Cyan)),
                    ]));
                    if let Ok(json_str) = serde_json::to_string_pretty(data) {
                        for json_line in json_str.lines() {
                            lines.push(Line::from(vec![
                                Span::raw("    "),
                                Span::styled(json_line.to_string(), Style::default().fg(Color::Cyan)),
                            ]));
                        }
                    }
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled("}", Style::default().fg(Color::Cyan)),
                    ]));
                }
                ToolResultContent::Error { message } => {
                    let wrapped = textwrap::wrap(message, width.saturating_sub(2) as usize);
                    for line in wrapped {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(line.to_string(), Style::default().fg(Color::Red)),
                        ]));
                    }
                }
                _ => {}
            }
        }

        lines
    }
}

impl Widget for ToolResultWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (icon, color) = if self.is_error {
            ("✖", Color::Red)
        } else {
            ("✓", Color::Green)
        };

        let header_style = Style::default().fg(color).add_modifier(Modifier::BOLD);

        let mut lines = vec![Line::from(vec![
            Span::styled(format!("{} ", icon), header_style),
            Span::styled("Result", header_style),
        ])];

        lines.extend(self.render_content(area.width));
        lines.push(Line::from(""));

        let text = Text::from(lines);
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

/// Widget for rendering progress messages.
#[derive(Debug, Clone)]
pub struct ProgressMessageWidget {
    /// The progress message.
    pub message: String,
    /// The tool use ID.
    pub tool_use_id: String,
    /// Progress type.
    pub progress_type: ProgressType,
}

/// Types of progress messages.
#[derive(Debug, Clone)]
pub enum ProgressType {
    /// Generic tool progress.
    Tool,
    /// Bash command progress.
    Bash { command: String },
    /// File read progress.
    FileRead { path: String, bytes_read: u64, total_bytes: u64 },
    /// MCP operation progress.
    Mcp { server_name: String, operation: String },
}

impl ProgressMessageWidget {
    /// Create a new progress message widget from ProgressData.
    #[must_use]
    pub fn from_progress_data(data: &ProgressData, tool_use_id: String) -> Self {
        let (message, progress_type) = match data {
            ProgressData::ToolProgress { message } => {
                (message.clone(), ProgressType::Tool)
            }
            ProgressData::Bash { command, output } => {
                let msg = format!(
                    "Running: {} {}",
                    command,
                    output.as_ref().map_or(String::new(), |o| format!("({} bytes)", o.len()))
                );
                (msg, ProgressType::Bash { command: command.clone() })
            }
            ProgressData::FileRead { path, bytes_read, total_bytes } => {
                let percent = if *total_bytes > 0 {
                    (*bytes_read * 100 / total_bytes) as u32
                } else {
                    0
                };
                let msg = format!("Reading {}: {}%", path, percent);
                (msg, ProgressType::FileRead {
                    path: path.clone(),
                    bytes_read: *bytes_read,
                    total_bytes: *total_bytes,
                })
            }
            ProgressData::Mcp { server_name, operation } => {
                let msg = format!("MCP {}: {}", server_name, operation);
                (msg, ProgressType::Mcp {
                    server_name: server_name.clone(),
                    operation: operation.clone(),
                })
            }
            _ => ("Working...".to_string(), ProgressType::Tool),
        };

        Self {
            message,
            tool_use_id,
            progress_type,
        }
    }

    /// Create from a runtime ProgressMessage.
    #[must_use]
    pub fn from_message(message: &runtime::messages::ProgressMessage) -> Self {
        Self::from_progress_data(&message.data, format!("{:?}", message.tool_use_id))
    }

    /// Calculate height for the given width.
    #[must_use]
    pub fn height(&self, width: u16) -> u16 {
        let lines = textwrap::wrap(&self.message, width.saturating_sub(4) as usize);
        lines.len() as u16 + 1
    }
}

impl Widget for ProgressMessageWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let spinner = "⠋"; // Static spinner for progress
        let style = Style::default().fg(Color::Cyan);

        let lines = vec![Line::from(vec![
            Span::styled(format!("{} ", spinner), style),
            Span::styled(self.message.clone(), style.add_modifier(Modifier::ITALIC)),
        ])];

        let text = Text::from(lines);
        Paragraph::new(text).render(area, buf);
    }
}

/// Markdown renderer for assistant messages.
pub mod markdown {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    use ratatui::text::{Line, Span, Text};

    /// Render markdown text to ratatui Text.
    pub fn render_markdown(input: &str) -> Text<'_> {
        let mut lines: Vec<Line<'_>> = Vec::new();
        let mut current_spans: Vec<Span<'_>> = Vec::new();
        let mut in_code_block = false;
        let mut code_language = String::new();
        let mut code_content = String::new();

        let parser = Parser::new(input);

        for event in parser {
            match event {
                Event::Start(tag) => match tag {
                    Tag::CodeBlock(kind) => {
                        in_code_block = true;
                        code_language = match kind {
                            pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
                            pulldown_cmark::CodeBlockKind::Indented => String::new(),
                        };
                    }
                    Tag::Strong => {
                        // Will apply bold to next text
                    }
                    Tag::Emphasis => {
                        // Will apply italic to next text
                    }
                    _ => {}
                },
                Event::End(tag_end) => match tag_end {
                    TagEnd::CodeBlock => {
                        in_code_block = false;
                        // Render code block
                        if !code_content.is_empty() {
                            lines.push(Line::from(Span::styled(
                                format!("```{}", code_language),
                                ratatui::style::Style::default()
                                    .fg(ratatui::style::Color::Gray),
                            )));
                            for line in code_content.lines() {
                                lines.push(Line::from(Span::styled(
                                    line.to_string(),
                                    ratatui::style::Style::default()
                                        .fg(ratatui::style::Color::White)
                                        .bg(ratatui::style::Color::Black),
                                )));
                            }
                            lines.push(Line::from(Span::styled(
                                "```",
                                ratatui::style::Style::default()
                                    .fg(ratatui::style::Color::Gray),
                            )));
                            code_content.clear();
                            code_language.clear();
                        }
                    }
                    TagEnd::Paragraph => {
                        if !current_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_spans)));
                        }
                        lines.push(Line::from(""));
                    }
                    _ => {}
                },
                Event::Text(text) => {
                    if in_code_block {
                        code_content.push_str(&text);
                    } else {
                        current_spans.push(Span::raw(text.to_string()));
                    }
                }
                Event::Code(code) => {
                    current_spans.push(Span::styled(
                        format!("`{}`", code),
                        ratatui::style::Style::default()
                            .fg(ratatui::style::Color::Yellow)
                            .bg(ratatui::style::Color::Black),
                    ));
                }
                Event::SoftBreak | Event::HardBreak => {
                    if !current_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_spans)));
                    }
                }
                _ => {}
            }
        }

        // Add any remaining spans
        if !current_spans.is_empty() {
            lines.push(Line::from(current_spans));
        }

        Text::from(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_message_widget() {
        let widget = UserMessageWidget::new("Hello, world!");
        assert_eq!(widget.content, "Hello, world!");
        assert!(widget.show_bubble);
    }

    #[test]
    fn test_system_message_widget() {
        let widget = SystemMessageWidget::new("Test message", MessageLevel::Info);
        assert_eq!(widget.content, "Test message");
        assert_eq!(widget.level_color(), Color::Blue);
        assert_eq!(widget.level_icon(), "ℹ");
    }

    #[test]
    fn test_system_message_warn() {
        let widget = SystemMessageWidget::new("Warning!", MessageLevel::Warn);
        assert_eq!(widget.level_color(), Color::Yellow);
        assert_eq!(widget.level_icon(), "⚠");
    }

    #[test]
    fn test_system_message_error() {
        let widget = SystemMessageWidget::new("Error!", MessageLevel::Error);
        assert_eq!(widget.level_color(), Color::Red);
        assert_eq!(widget.level_icon(), "✖");
    }

    #[test]
    fn test_tool_call_widget() {
        let input = serde_json::json!({"path": "/tmp/test.txt"});
        let widget = ToolCallWidget::new("Read", "tool_123", input);
        assert_eq!(widget.tool_name, "Read");
        assert!(widget.is_running);
    }

    #[test]
    fn test_markdown_render() {
        let text = "Hello **world**";
        let result = markdown::render_markdown(text);
        assert!(!result.lines.is_empty());
    }
}
