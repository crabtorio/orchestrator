pub enum LogTarget {
    // ── asteroids & sunrays ──────────────────────────────────────────────
    /// Fallback when the event kind cannot be inferred from content
    AsteroidsSunrays,
    Asteroids,
    Sunrays,
    // ── conversations ────────────────────────────────────────────────────
    /// Fallback for queue/scheduler infrastructure messages
    Conversations,
    ConversationsPlanets,
    ConversationsExplorers,
    // ── channel messages ─────────────────────────────────────────────────
    /// Fallback when the message direction cannot be inferred
    ChannelMessages,
    ChannelMessagesPlanets,
    ChannelMessagesExplorers,
    ChannelMessagesUi,
    // ── lifecycle & other ────────────────────────────────────────────────
    General,
    PlanetLifecycle,
    ExplorerLifecycle,
    OrchestratorLifecycle,
}

//TODO
pub fn log_internal() {}