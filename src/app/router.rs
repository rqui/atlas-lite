//! Hierarchical screen router for the portrait product UI shell.

/// Atlas product routes, kept separate from the legacy RustMix screen tree
/// while the product shell is introduced incrementally.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AtlasRoute {
    #[default]
    Home,
    Library,
    Note,
    Search,
    Views,
    Capture,
    Settings,
}

static ATLAS_HOME_MENU_ROUTES: [AtlasRoute; 5] = [
    AtlasRoute::Library,
    AtlasRoute::Search,
    AtlasRoute::Views,
    AtlasRoute::Capture,
    AtlasRoute::Settings,
];

impl AtlasRoute {
    /// Atlas Home navigation entries. Note is deliberately absent because it
    /// is opened from another Atlas surface.
    #[must_use]
    pub const fn home_menu_routes() -> &'static [Self] {
        &ATLAS_HOME_MENU_ROUTES
    }

    /// Static parents for Atlas routes. Note has no static parent because its
    /// return surface is preserved by [`ScreenRouter`].
    #[must_use]
    pub const fn parent(self) -> Option<Self> {
        match self {
            Self::Home | Self::Note => None,
            Self::Library | Self::Search | Self::Views | Self::Capture | Self::Settings => {
                Some(Self::Home)
            }
        }
    }
}

/// Product screens exposed by the RustMix Wave shell.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScreenRoute {
    #[default]
    Home,
    Reader,
    Productivity,
    Games,
    Tools,
    Settings,
    ContinueReading,
    Library,
    Bookmarks,
    ReaderBookmarks,
    ReaderLoading,
    ReaderPage,
    ReaderOptions,
    ReaderPreferences,
    ReaderToc,
    Calendar,
    CalendarAgenda,
    CalendarEventDetails,
    CalendarEventEditor,
    CalendarDeleteConfirmation,
    VoiceNotes,
    VoiceNoteDetails,
    VoiceNoteRecording,
    GamesTbd,
    LuaApps,
    LuaGame,
    LuaGameError,
    Files,
    Dictionary,
    UnitConverter,
    Alarms,
    Audio,
    AudioDetails,
    Clock,
    ClockDetails,
    Display,
    PowerKeyMenu,
    DeviceInfo,
    DeviceInfoBoard,
    DeviceInfoRuntime,
    Environment,
    EnvironmentDetails,
    Motion,
    MotionEvents,
    MotionDetails,
    Network,
    NetworkDetails,
    WifiTransfer,
    Weather,
    WeatherDetails,
}

impl ScreenRoute {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Reader => "Reader",
            Self::Productivity => "Productivity",
            Self::Games => "Games",
            Self::Tools => "Tools",
            Self::Settings => "Settings",
            Self::ContinueReading => "Continue Reading",
            Self::Library => "Library",
            Self::Bookmarks => "Bookmarks",
            Self::ReaderBookmarks => "Reader Bookmarks",
            Self::ReaderLoading => "Opening Book",
            Self::ReaderPage => "Reader Page",
            Self::ReaderOptions => "Reader Options",
            Self::ReaderPreferences => "Reading Preferences",
            Self::ReaderToc => "Table of Contents",
            Self::Calendar => "Calendar",
            Self::CalendarAgenda => "Daily Agenda",
            Self::CalendarEventDetails => "Calendar Event",
            Self::CalendarEventEditor => "Edit Calendar Event",
            Self::CalendarDeleteConfirmation => "Delete Calendar Event",
            Self::VoiceNotes => "Voice Notes",
            Self::VoiceNoteDetails => "Voice Note",
            Self::VoiceNoteRecording => "Record Voice Note",
            Self::GamesTbd => "TBD",
            Self::LuaApps => "SD Lua Apps",
            Self::LuaGame => "Lua App",
            Self::LuaGameError => "Lua App Error",
            Self::Files => "File Browser",
            Self::Dictionary => "Dictionary",
            Self::UnitConverter => "Unit Converter",
            Self::Alarms => "Alarms",
            Self::Audio => "Audio",
            Self::AudioDetails => "Audio details",
            Self::Clock => "Clock",
            Self::ClockDetails => "RTC details",
            Self::Display => "Display",
            Self::PowerKeyMenu => "Power Key Menu",
            Self::DeviceInfo => "Device Info",
            Self::DeviceInfoBoard => "Board services",
            Self::DeviceInfoRuntime => "Runtime services",
            Self::Environment => "Environment",
            Self::EnvironmentDetails => "Sensor details",
            Self::Motion => "Motion",
            Self::MotionEvents => "Motion events",
            Self::MotionDetails => "Motion details",
            Self::Network => "Network",
            Self::NetworkDetails => "Provisioning details",
            Self::WifiTransfer => "Wi-Fi Transfer",
            Self::Weather => "Weather",
            Self::WeatherDetails => "Weather details",
        }
    }

    #[must_use]
    pub const fn marker(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Reader => "reader",
            Self::Productivity => "productivity",
            Self::Games => "games",
            Self::Tools => "tools",
            Self::Settings => "settings",
            Self::ContinueReading => "continue-reading",
            Self::Library => "library",
            Self::Bookmarks => "bookmarks",
            Self::ReaderBookmarks => "reader-bookmarks",
            Self::ReaderLoading => "reader-loading",
            Self::ReaderPage => "reader-page",
            Self::ReaderOptions => "reader-options",
            Self::ReaderPreferences => "reader-preferences",
            Self::ReaderToc => "reader-toc",
            Self::Calendar => "calendar",
            Self::CalendarAgenda => "calendar-agenda",
            Self::CalendarEventDetails => "calendar-event-details",
            Self::CalendarEventEditor => "calendar-event-editor",
            Self::CalendarDeleteConfirmation => "calendar-delete-confirmation",
            Self::VoiceNotes => "voice-notes",
            Self::VoiceNoteDetails => "voice-note-details",
            Self::VoiceNoteRecording => "voice-note-recording",
            Self::GamesTbd => "games-tbd",
            Self::LuaApps => "lua-apps",
            Self::LuaGame => "lua-game",
            Self::LuaGameError => "lua-game-error",
            Self::Files => "file-browser",
            Self::Dictionary => "dictionary",
            Self::UnitConverter => "unit-converter",
            Self::Alarms => "alarms",
            Self::Audio => "audio",
            Self::AudioDetails => "audio-details",
            Self::Clock => "clock",
            Self::ClockDetails => "rtc-details",
            Self::Display => "display",
            Self::PowerKeyMenu => "power-key-menu",
            Self::DeviceInfo => "device-info",
            Self::DeviceInfoBoard => "device-info-board",
            Self::DeviceInfoRuntime => "device-info-runtime",
            Self::Environment => "environment",
            Self::EnvironmentDetails => "environment-details",
            Self::Motion => "motion",
            Self::MotionEvents => "motion-events",
            Self::MotionDetails => "motion-details",
            Self::Network => "network",
            Self::NetworkDetails => "network-details",
            Self::WifiTransfer => "wifi-transfer",
            Self::Weather => "weather",
            Self::WeatherDetails => "weather-details",
        }
    }

    #[must_use]
    pub const fn is_category(self) -> bool {
        matches!(
            self,
            Self::Reader | Self::Productivity | Self::Games | Self::Tools | Self::Settings
        )
    }

    #[must_use]
    pub const fn is_placeholder(self) -> bool {
        matches!(self, Self::GamesTbd)
    }

    #[must_use]
    pub const fn parent(self) -> Option<Self> {
        match self {
            Self::Home => None,
            Self::Reader | Self::Productivity | Self::Games | Self::Tools | Self::Settings => {
                Some(Self::Home)
            }
            Self::ContinueReading | Self::Library | Self::Bookmarks => Some(Self::Reader),
            Self::ReaderBookmarks => Some(Self::ReaderOptions),
            Self::ReaderLoading | Self::ReaderPage => Some(Self::Library),
            Self::ReaderOptions => Some(Self::ReaderPage),
            Self::ReaderPreferences => Some(Self::ReaderOptions),
            Self::ReaderToc => Some(Self::ReaderOptions),
            Self::Calendar | Self::VoiceNotes => Some(Self::Productivity),
            Self::CalendarAgenda => Some(Self::Calendar),
            Self::CalendarEventDetails => Some(Self::CalendarAgenda),
            Self::CalendarEventEditor => Some(Self::CalendarAgenda),
            Self::CalendarDeleteConfirmation => Some(Self::CalendarEventDetails),
            Self::VoiceNoteDetails | Self::VoiceNoteRecording => Some(Self::VoiceNotes),
            Self::GamesTbd | Self::LuaApps => Some(Self::Games),
            Self::LuaGame | Self::LuaGameError => Some(Self::LuaApps),
            Self::Files | Self::Dictionary | Self::UnitConverter => Some(Self::Tools),
            Self::PowerKeyMenu => Some(Self::Home),
            Self::Alarms
            | Self::Audio
            | Self::Clock
            | Self::Display
            | Self::DeviceInfo
            | Self::Environment
            | Self::Motion
            | Self::Network
            | Self::Weather => Some(Self::Settings),
            Self::AudioDetails => Some(Self::Audio),
            Self::ClockDetails => Some(Self::Clock),
            Self::DeviceInfoBoard => Some(Self::DeviceInfo),
            Self::DeviceInfoRuntime => Some(Self::DeviceInfoBoard),
            Self::EnvironmentDetails => Some(Self::Environment),
            Self::MotionEvents => Some(Self::Motion),
            Self::MotionDetails => Some(Self::MotionEvents),
            Self::NetworkDetails | Self::WifiTransfer => Some(Self::Network),
            Self::WeatherDetails => Some(Self::Weather),
        }
    }

    #[must_use]
    pub const fn uses_live_status(self) -> bool {
        matches!(
            self,
            Self::Clock
                | Self::ClockDetails
                | Self::Environment
                | Self::EnvironmentDetails
                | Self::Motion
                | Self::MotionDetails
                | Self::Network
                | Self::NetworkDetails
                | Self::WifiTransfer
                | Self::Alarms
                | Self::Calendar
                | Self::CalendarAgenda
                | Self::ReaderLoading
                | Self::VoiceNoteRecording
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScreenRouter {
    current: ScreenRoute,
    atlas_current: AtlasRoute,
    atlas_note_return_route: Option<AtlasRoute>,
}

impl ScreenRouter {
    #[must_use]
    pub const fn current(self) -> ScreenRoute {
        self.current
    }

    /// Current route in the Atlas product navigation shell.
    #[must_use]
    pub const fn atlas_current(self) -> AtlasRoute {
        self.atlas_current
    }

    pub fn navigate_to(&mut self, route: ScreenRoute) {
        self.current = route;
    }

    /// Navigate directly to an Atlas surface, clearing any Note return
    /// context that cannot apply to the new destination.
    pub fn navigate_atlas_to(&mut self, route: AtlasRoute) {
        self.atlas_current = route;
        self.atlas_note_return_route = None;
    }

    /// Open an Atlas Note while retaining the surface that initiated it.
    pub fn open_atlas_note_from(&mut self, opening_surface: AtlasRoute) {
        self.atlas_current = AtlasRoute::Note;
        self.atlas_note_return_route = Some(opening_surface);
    }

    pub fn back(&mut self) {
        self.current = self.current.parent().unwrap_or(ScreenRoute::Home);
    }

    /// Apply hierarchical Back inside the Atlas product navigation shell.
    pub fn atlas_back(&mut self) {
        self.atlas_current = if self.atlas_current == AtlasRoute::Note {
            self.atlas_note_return_route
                .take()
                .unwrap_or(AtlasRoute::Home)
        } else {
            self.atlas_current.parent().unwrap_or(AtlasRoute::Home)
        };
    }

    pub fn back_home(&mut self) {
        self.current = ScreenRoute::Home;
    }
}

#[cfg(test)]
mod tests {
    use super::{AtlasRoute, ScreenRoute, ScreenRouter};

    #[test]
    fn atlas_product_root_exposes_only_atlas_navigation_routes() {
        assert_eq!(
            AtlasRoute::home_menu_routes(),
            &[
                AtlasRoute::Library,
                AtlasRoute::Search,
                AtlasRoute::Views,
                AtlasRoute::Capture,
                AtlasRoute::Settings,
            ]
        );
        assert!(!AtlasRoute::home_menu_routes().contains(&AtlasRoute::Note));
    }

    #[test]
    fn atlas_product_routes_return_to_home() {
        assert_eq!(AtlasRoute::Home.parent(), None);

        for route in [
            AtlasRoute::Library,
            AtlasRoute::Search,
            AtlasRoute::Views,
            AtlasRoute::Capture,
            AtlasRoute::Settings,
        ] {
            assert_eq!(route.parent(), Some(AtlasRoute::Home));

            let mut router = ScreenRouter::default();
            router.navigate_atlas_to(route);
            router.atlas_back();
            assert_eq!(router.atlas_current(), AtlasRoute::Home);
        }
    }

    #[test]
    fn atlas_note_returns_to_the_opening_surface() {
        let mut router = ScreenRouter::default();
        assert_eq!(AtlasRoute::Note.parent(), None);

        for opening_surface in [AtlasRoute::Library, AtlasRoute::Search, AtlasRoute::Views] {
            router.navigate_atlas_to(opening_surface);
            router.open_atlas_note_from(opening_surface);
            assert_eq!(router.atlas_current(), AtlasRoute::Note);

            router.atlas_back();
            assert_eq!(router.atlas_current(), opening_surface);
        }
    }

    #[test]
    fn router_exposes_static_parent_hierarchy() {
        assert_eq!(ScreenRoute::Files.parent(), Some(ScreenRoute::Tools));
        assert_eq!(ScreenRoute::Display.parent(), Some(ScreenRoute::Settings));
        assert_eq!(ScreenRoute::PowerKeyMenu.parent(), Some(ScreenRoute::Home));
        assert_eq!(
            ScreenRoute::Calendar.parent(),
            Some(ScreenRoute::Productivity)
        );
        assert_eq!(
            ScreenRoute::CalendarAgenda.parent(),
            Some(ScreenRoute::Calendar)
        );
        assert_eq!(
            ScreenRoute::CalendarEventDetails.parent(),
            Some(ScreenRoute::CalendarAgenda)
        );
        assert_eq!(
            ScreenRoute::CalendarEventEditor.parent(),
            Some(ScreenRoute::CalendarAgenda)
        );
        assert_eq!(
            ScreenRoute::CalendarDeleteConfirmation.parent(),
            Some(ScreenRoute::CalendarEventDetails)
        );
        assert_eq!(
            ScreenRoute::UnitConverter.parent(),
            Some(ScreenRoute::Tools)
        );
        assert!(!ScreenRoute::UnitConverter.is_placeholder());
        assert!(!ScreenRoute::Dictionary.is_placeholder());
        assert_eq!(ScreenRoute::AudioDetails.parent(), Some(ScreenRoute::Audio));
        assert_eq!(
            ScreenRoute::DeviceInfoRuntime.parent(),
            Some(ScreenRoute::DeviceInfoBoard)
        );
        assert_eq!(ScreenRoute::Reader.parent(), Some(ScreenRoute::Home));
        assert_eq!(ScreenRoute::LuaApps.parent(), Some(ScreenRoute::Games));
        assert_eq!(ScreenRoute::LuaGame.parent(), Some(ScreenRoute::LuaApps));
        assert_eq!(
            ScreenRoute::LuaGameError.parent(),
            Some(ScreenRoute::LuaApps)
        );
        assert_eq!(ScreenRoute::Home.parent(), None);
    }

    #[test]
    fn back_returns_details_to_overview_then_category_then_home() {
        let mut router = ScreenRouter::default();
        router.navigate_to(ScreenRoute::Settings);
        router.navigate_to(ScreenRoute::Audio);
        router.navigate_to(ScreenRoute::AudioDetails);
        router.back();
        assert_eq!(router.current(), ScreenRoute::Audio);
        router.back();
        assert_eq!(router.current(), ScreenRoute::Settings);
        router.back();
        assert_eq!(router.current(), ScreenRoute::Home);
    }
}
