use soroban_sdk::{contracttype, Address, String, Env};

/// Match result enumeration for clear result encoding
/// 
/// Represents the possible outcomes of a prediction match
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatchResult {
    /// Team A wins (team_a field)
    TeamAWins,
    /// Team B wins (team_b field)  
    TeamBWins,
    /// Match ends in a draw/tie
    Draw,
}

impl MatchResult {
    /// Convert MatchResult to numeric encoding for storage efficiency
    /// 
    /// # Returns
    /// * `u32` - 0 for TeamAWins, 1 for TeamBWins, 2 for Draw
    pub fn to_u32(&self) -> u32 {
        match self {
            MatchResult::TeamAWins => 0,
            MatchResult::TeamBWins => 1,
            MatchResult::Draw => 2,
        }
    }

    /// Convert numeric encoding back to MatchResult
    /// 
    /// # Arguments
    /// * `value` - Numeric value (0=TeamA, 1=TeamB, 2=Draw)
    /// 
    /// # Returns
    /// * `Option<MatchResult>` - Some(result) if valid, None if invalid
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(MatchResult::TeamAWins),
            1 => Some(MatchResult::TeamBWins),
            2 => Some(MatchResult::Draw),
            _ => None,
        }
    }
}

/// Individual prediction match within an event
/// 
/// Represents a specific match that users can place predictions on.
/// Each event can contain multiple matches, allowing for complex prediction scenarios.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Match {
    /// Unique identifier for this match
    /// Should be unique across all matches in the system
    pub match_id: u64,

    /// ID of the event this match belongs to
    /// Links the match to its parent prediction event
    pub event_id: u64,

    /// Name/identifier of the first team or option
    /// Examples: "Team Red", "Yes", "Bitcoin", "Candidate A"
    pub team_a: String,

    /// Name/identifier of the second team or option  
    /// Examples: "Team Blue", "No", "Ethereum", "Candidate B"
    pub team_b: String,

    /// Scheduled time for the match (Unix timestamp in seconds)
    /// When the actual event/match is expected to occur
    pub match_time: u64,

    /// Whether a result has been submitted for this match
    /// true = result is available, false = still pending
    pub result_submitted: bool,

    /// The winning outcome of the match
    /// None = no result yet, Some(0) = Team A wins, Some(1) = Team B wins, Some(2) = Draw
    pub winning_team: Option<u32>,

    /// Timestamp when the result was submitted (Unix timestamp in seconds)
    /// None = no result submitted yet, Some(timestamp) = when result was recorded
    pub result_timestamp: Option<u64>,
}

impl Match {
    /// Create a new Match with default values
    /// 
    /// # Arguments
    /// * `match_id` - Unique identifier for the match
    /// * `event_id` - ID of the parent event
    /// * `team_a` - Name of the first team/option
    /// * `team_b` - Name of the second team/option
    /// * `match_time` - Scheduled match time (Unix timestamp)
    /// 
    /// # Returns
    /// * `Match` - New match instance with no result submitted
    pub fn new(
        match_id: u64,
        event_id: u64,
        team_a: String,
        team_b: String,
        match_time: u64,
    ) -> Self {
        Self {
            match_id,
            event_id,
            team_a,
            team_b,
            match_time,
            result_submitted: false,
            winning_team: None,
            result_timestamp: None,
        }
    }

    /// Submit a result for this match
    /// 
    /// # Arguments
    /// * `result` - The match result (TeamAWins, TeamBWins, or Draw)
    /// * `timestamp` - When the result was determined
    /// 
    /// # Returns
    /// * `Result<(), &'static str>` - Ok if successful, Err if result already submitted
    pub fn submit_result(&mut self, result: MatchResult, timestamp: u64) -> Result<(), &'static str> {
        if self.result_submitted {
            return Err("Result already submitted for this match");
        }

        self.winning_team = Some(result.to_u32());
        self.result_timestamp = Some(timestamp);
        self.result_submitted = true;

        Ok(())
    }

    /// Get the match result as a MatchResult enum
    /// 
    /// # Returns
    /// * `Option<MatchResult>` - Some(result) if available, None if no result yet
    pub fn get_result(&self) -> Option<MatchResult> {
        self.winning_team.and_then(MatchResult::from_u32)
    }

    /// Check if the match has started (current time >= match_time)
    /// 
    /// # Arguments
    /// * `current_time` - Current Unix timestamp
    /// 
    /// # Returns
    /// * `bool` - true if match has started or is in progress
    pub fn has_started(&self, current_time: u64) -> bool {
        current_time >= self.match_time
    }

    /// Check if the match is ready for result submission
    /// 
    /// A match is ready for results if it has started but no result has been submitted yet
    /// 
    /// # Arguments
    /// * `current_time` - Current Unix timestamp
    /// 
    /// # Returns
    /// * `bool` - true if ready for result submission
    pub fn is_ready_for_result(&self, current_time: u64) -> bool {
        self.has_started(current_time) && !self.result_submitted
    }

    /// Get time until match starts (0 if already started)
    /// 
    /// # Arguments
    /// * `current_time` - Current Unix timestamp
    /// 
    /// # Returns
    /// * `u64` - Seconds until match starts, 0 if already started
    pub fn time_until_start(&self, current_time: u64) -> u64 {
        if current_time >= self.match_time {
            0
        } else {
            self.match_time - current_time
        }
    }

    /// Get time since result was submitted (0 if no result)
    /// 
    /// # Arguments
    /// * `current_time` - Current Unix timestamp
    /// 
    /// # Returns
    /// * `u64` - Seconds since result submission, 0 if no result
    pub fn time_since_result(&self, current_time: u64) -> u64 {
        match self.result_timestamp {
            Some(result_time) => current_time.saturating_sub(result_time),
            None => 0,
        }
    }

    /// Validate match data for consistency and constraints
    /// 
    /// # Returns
    /// * `Result<(), &'static str>` - Ok if valid, Err with description if invalid
    pub fn validate(&self) -> Result<(), &'static str> {
        // Check that team names are not empty
        if self.team_a.len() == 0 {
            return Err("Team A name cannot be empty");
        }
        
        if self.team_b.len() == 0 {
            return Err("Team B name cannot be empty");
        }

        // Check that team names are different
        if self.team_a == self.team_b {
            return Err("Team names must be different");
        }

        // Validate result consistency
        if self.result_submitted {
            if self.winning_team.is_none() {
                return Err("Result submitted but no winning team specified");
            }
            if self.result_timestamp.is_none() {
                return Err("Result submitted but no timestamp specified");
            }
            // Validate winning team value
            if let Some(team) = self.winning_team {
                if team > 2 {
                    return Err("Invalid winning team value (must be 0, 1, or 2)");
                }
            }
        } else {
            if self.winning_team.is_some() {
                return Err("Winning team specified but result not marked as submitted");
            }
            if self.result_timestamp.is_some() {
                return Err("Result timestamp specified but result not marked as submitted");
            }
        }

        Ok(())
    }

    /// Check if this match allows predictions at the current time
    /// 
    /// Typically, predictions should close before the match starts
    /// 
    /// # Arguments
    /// * `current_time` - Current Unix timestamp
    /// * `prediction_cutoff_minutes` - Minutes before match when predictions close
    /// 
    /// # Returns
    /// * `bool` - true if predictions are still allowed
    pub fn allows_predictions(&self, current_time: u64, prediction_cutoff_minutes: u64) -> bool {
        let cutoff_time = self.match_time.saturating_sub(prediction_cutoff_minutes * 60);
        current_time < cutoff_time && !self.result_submitted
    }
}

/// Core Event struct that stores all information about a creator's prediction event
/// 
/// This struct is optimized for Soroban storage and contains all necessary metadata
/// for managing creator-hosted prediction events with XLM token fees.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    /// Unique identifier for the event
    /// Auto-incremented when events are created
    pub event_id: u64,

    /// Address of the creator who owns this event
    /// Only the creator can resolve the event and distribute winnings
    pub creator: Address,

    /// Display name/title of the prediction event
    /// Should be descriptive and user-friendly (e.g., "Bitcoin Price Prediction")
    pub name: String,

    /// Detailed description of the event and prediction criteria
    /// Contains rules, conditions, and any additional context for participants
    pub description: String,

    /// Entry fee required to participate in XLM tokens (stroops)
    /// All participants must pay this amount to place a prediction
    /// Example: 1000000 stroops = 0.1 XLM
    pub creation_fee: i128,

    /// Timestamp when the event was created (Unix timestamp in seconds)
    /// Used for event ordering and historical tracking
    pub created_at: u64,

    /// Whether the event is currently active and accepting predictions
    /// - true: Event is open for predictions
    /// - false: Event is closed, resolved, or cancelled
    pub is_active: bool,

    /// Current number of participants who have placed predictions
    /// Incremented each time a new user places a prediction
    pub total_participants: u32,

    /// Total number of prediction matches/options available
    /// Represents the number of possible outcomes users can bet on
    /// Example: For "Yes/No" prediction, this would be 2
    pub total_matches: u32,
}

impl Event {
    /// Create a new Event with default values
    /// 
    /// # Arguments
    /// * `event_id` - Unique identifier for the event
    /// * `creator` - Address of the event creator
    /// * `name` - Display name of the event
    /// * `description` - Detailed description of the event
    /// * `creation_fee` - Entry fee in XLM stroops
    /// * `created_at` - Creation timestamp
    /// * `total_matches` - Number of prediction options available
    /// 
    /// # Returns
    /// * `Event` - New event instance with is_active=true and total_participants=0
    pub fn new(
        event_id: u64,
        creator: Address,
        name: String,
        description: String,
        creation_fee: i128,
        created_at: u64,
        total_matches: u32,
    ) -> Self {
        Self {
            event_id,
            creator,
            name,
            description,
            creation_fee,
            created_at,
            is_active: true,
            total_participants: 0,
            total_matches,
        }
    }

    /// Check if the event is currently accepting predictions
    /// 
    /// # Returns
    /// * `bool` - true if event is active and can accept new predictions
    pub fn can_accept_predictions(&self) -> bool {
        self.is_active && self.total_participants < u32::MAX
    }

    /// Deactivate the event (close it for new predictions)
    /// 
    /// This is typically called when the event is resolved or cancelled
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// Add a new participant to the event
    /// 
    /// Increments the total_participants counter
    /// Should be called when a user successfully places a prediction
    /// 
    /// # Returns
    /// * `Result<(), &'static str>` - Ok if successful, Err if event is full
    pub fn add_participant(&mut self) -> Result<(), &'static str> {
        if !self.is_active {
            return Err("Event is not active");
        }
        
        if self.total_participants >= u32::MAX {
            return Err("Event is full");
        }
        
        self.total_participants += 1;
        Ok(())
    }

    /// Get the total prize pool for this event
    /// 
    /// Calculates the total amount of XLM collected from all participants
    /// 
    /// # Returns
    /// * `i128` - Total prize pool in XLM stroops
    pub fn get_total_prize_pool(&self) -> i128 {
        self.creation_fee.saturating_mul(self.total_participants as i128)
    }

    /// Check if the event has any participants
    /// 
    /// # Returns
    /// * `bool` - true if at least one user has placed a prediction
    pub fn has_participants(&self) -> bool {
        self.total_participants > 0
    }

    /// Get event age in seconds
    /// 
    /// # Arguments
    /// * `current_timestamp` - Current Unix timestamp
    /// 
    /// # Returns
    /// * `u64` - Age of the event in seconds
    pub fn get_age_seconds(&self, current_timestamp: u64) -> u64 {
        current_timestamp.saturating_sub(self.created_at)
    }
}

/// Event status enumeration for more granular state management
/// 
/// This provides more detailed status information beyond the simple is_active boolean
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventStatus {
    /// Event is active and accepting predictions
    Active,
    /// Event is closed for new predictions but not yet resolved
    Closed,
    /// Event has been resolved with a winning outcome
    Resolved,
    /// Event has been cancelled (refunds may be issued)
    Cancelled,
    /// Event is paused temporarily
    Paused,
}

/// Extended event metadata for additional information
/// 
/// This struct can be used for storing additional event data that doesn't
/// need to be in the core Event struct for storage optimization
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventMetadata {
    /// Event category or type (e.g., "Sports", "Crypto", "Politics")
    pub category: String,
    
    /// Tags for event discovery and filtering
    pub tags: String, // Comma-separated tags for simplicity
    
    /// Minimum number of participants required for the event to be valid
    pub min_participants: u32,
    
    /// Maximum number of participants allowed
    pub max_participants: u32,
    
    /// Event end timestamp (when predictions close)
    pub end_time: u64,
    
    /// Resolution timestamp (when results are determined)
    pub resolution_time: u64,
    
    /// Whether the event is invite-only
    pub is_invite_only: bool,
    
    /// Creator's reputation score at time of event creation
    pub creator_reputation: u32,
}

impl EventMetadata {
    /// Create new event metadata with default values
    pub fn new(
        category: String,
        tags: String,
        min_participants: u32,
        max_participants: u32,
        end_time: u64,
        resolution_time: u64,
        is_invite_only: bool,
        creator_reputation: u32,
    ) -> Self {
        Self {
            category,
            tags,
            min_participants,
            max_participants,
            end_time,
            resolution_time,
            is_invite_only,
            creator_reputation,
        }
    }

    /// Check if the event is currently in the prediction phase
    pub fn is_prediction_phase(&self, current_time: u64) -> bool {
        current_time < self.end_time
    }

    /// Check if the event is in the resolution phase
    pub fn is_resolution_phase(&self, current_time: u64) -> bool {
        current_time >= self.end_time && current_time < self.resolution_time
    }

    /// Check if the event should be automatically resolved
    pub fn should_auto_resolve(&self, current_time: u64) -> bool {
        current_time >= self.resolution_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, Address, String, testutils::Address as _};

    #[test]
    fn test_event_creation() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let name = String::from_str(&env, "Test Event");
        let description = String::from_str(&env, "A test prediction event");
        
        let event = Event::new(
            1,
            creator.clone(),
            name.clone(),
            description.clone(),
            1000000, // 0.1 XLM
            1640995200, // Jan 1, 2022
            2, // Yes/No prediction
        );

        assert_eq!(event.event_id, 1);
        assert_eq!(event.creator, creator);
        assert_eq!(event.name, name);
        assert_eq!(event.description, description);
        assert_eq!(event.creation_fee, 1000000);
        assert_eq!(event.created_at, 1640995200);
        assert_eq!(event.is_active, true);
        assert_eq!(event.total_participants, 0);
        assert_eq!(event.total_matches, 2);
    }

    #[test]
    fn test_event_participant_management() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let name = String::from_str(&env, "Test Event");
        let description = String::from_str(&env, "A test prediction event");
        
        let mut event = Event::new(1, creator, name, description, 1000000, 1640995200, 2);

        // Initially no participants
        assert_eq!(event.total_participants, 0);
        assert!(!event.has_participants());
        assert_eq!(event.get_total_prize_pool(), 0);

        // Add first participant
        assert!(event.add_participant().is_ok());
        assert_eq!(event.total_participants, 1);
        assert!(event.has_participants());
        assert_eq!(event.get_total_prize_pool(), 1000000);

        // Add second participant
        assert!(event.add_participant().is_ok());
        assert_eq!(event.total_participants, 2);
        assert_eq!(event.get_total_prize_pool(), 2000000);

        // Deactivate event
        event.deactivate();
        assert!(!event.is_active);
        assert!(!event.can_accept_predictions());

        // Cannot add participants to inactive event
        assert!(event.add_participant().is_err());
    }

    #[test]
    fn test_event_age_calculation() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let name = String::from_str(&env, "Test Event");
        let description = String::from_str(&env, "A test prediction event");
        
        let event = Event::new(1, creator, name, description, 1000000, 1640995200, 2);

        // Test age calculation
        let current_time = 1640995200 + 3600; // 1 hour later
        assert_eq!(event.get_age_seconds(current_time), 3600);

        // Test with earlier timestamp (should not underflow)
        let earlier_time = 1640995200 - 1000;
        assert_eq!(event.get_age_seconds(earlier_time), 0);
    }

    #[test]
    fn test_event_metadata() {
        let env = Env::default();
        let category = String::from_str(&env, "Sports");
        let tags = String::from_str(&env, "football,nfl,superbowl");
        
        let metadata = EventMetadata::new(
            category.clone(),
            tags.clone(),
            10, // min participants
            1000, // max participants
            1640995200 + 86400, // end time (24 hours later)
            1640995200 + 172800, // resolution time (48 hours later)
            false, // not invite only
            100, // creator reputation
        );

        assert_eq!(metadata.category, category);
        assert_eq!(metadata.tags, tags);
        assert_eq!(metadata.min_participants, 10);
        assert_eq!(metadata.max_participants, 1000);
        assert!(!metadata.is_invite_only);
        assert_eq!(metadata.creator_reputation, 100);

        // Test phase checking
        let prediction_time = 1640995200 + 43200; // 12 hours after creation
        assert!(metadata.is_prediction_phase(prediction_time));
        assert!(!metadata.is_resolution_phase(prediction_time));
        assert!(!metadata.should_auto_resolve(prediction_time));

        let resolution_time = 1640995200 + 129600; // 36 hours after creation
        assert!(!metadata.is_prediction_phase(resolution_time));
        assert!(metadata.is_resolution_phase(resolution_time));
        assert!(!metadata.should_auto_resolve(resolution_time));

        let auto_resolve_time = 1640995200 + 259200; // 72 hours after creation
        assert!(!metadata.is_prediction_phase(auto_resolve_time));
        assert!(!metadata.is_resolution_phase(auto_resolve_time));
        assert!(metadata.should_auto_resolve(auto_resolve_time));
    }
}
    #[test]
    fn test_match_creation() {
        let env = Env::default();
        let team_a = String::from_str(&env, "Team Alpha");
        let team_b = String::from_str(&env, "Team Beta");
        let match_time = 1640995200; // Jan 1, 2022
        
        let match_obj = Match::new(1, 100, team_a.clone(), team_b.clone(), match_time);

        assert_eq!(match_obj.match_id, 1);
        assert_eq!(match_obj.event_id, 100);
        assert_eq!(match_obj.team_a, team_a);
        assert_eq!(match_obj.team_b, team_b);
        assert_eq!(match_obj.match_time, match_time);
        assert_eq!(match_obj.result_submitted, false);
        assert_eq!(match_obj.winning_team, None);
        assert_eq!(match_obj.result_timestamp, None);
    }

    #[test]
    fn test_match_result_submission() {
        let env = Env::default();
        let team_a = String::from_str(&env, "Team Alpha");
        let team_b = String::from_str(&env, "Team Beta");
        
        let mut match_obj = Match::new(1, 100, team_a, team_b, 1640995200);

        // Submit result
        let result_time = 1640995200 + 7200; // 2 hours after match time
        assert!(match_obj.submit_result(MatchResult::TeamAWins, result_time).is_ok());

        assert_eq!(match_obj.result_submitted, true);
        assert_eq!(match_obj.winning_team, Some(0));
        assert_eq!(match_obj.result_timestamp, Some(result_time));
        assert_eq!(match_obj.get_result(), Some(MatchResult::TeamAWins));

        // Cannot submit result twice
        assert!(match_obj.submit_result(MatchResult::TeamBWins, result_time + 100).is_err());
    }

    #[test]
    fn test_match_result_encoding() {
        // Test MatchResult to u32 conversion
        assert_eq!(MatchResult::TeamAWins.to_u32(), 0);
        assert_eq!(MatchResult::TeamBWins.to_u32(), 1);
        assert_eq!(MatchResult::Draw.to_u32(), 2);

        // Test u32 to MatchResult conversion
        assert_eq!(MatchResult::from_u32(0), Some(MatchResult::TeamAWins));
        assert_eq!(MatchResult::from_u32(1), Some(MatchResult::TeamBWins));
        assert_eq!(MatchResult::from_u32(2), Some(MatchResult::Draw));
        assert_eq!(MatchResult::from_u32(3), None); // Invalid
        assert_eq!(MatchResult::from_u32(999), None); // Invalid
    }

    #[test]
    fn test_match_timing() {
        let env = Env::default();
        let team_a = String::from_str(&env, "Team Alpha");
        let team_b = String::from_str(&env, "Team Beta");
        let match_time = 1640995200;
        
        let match_obj = Match::new(1, 100, team_a, team_b, match_time);

        // Before match starts
        let before_time = match_time - 3600; // 1 hour before
        assert!(!match_obj.has_started(before_time));
        assert!(match_obj.is_ready_for_result(before_time) == false);
        assert_eq!(match_obj.time_until_start(before_time), 3600);
        assert_eq!(match_obj.time_since_result(before_time), 0);

        // After match starts
        let after_time = match_time + 1800; // 30 minutes after
        assert!(match_obj.has_started(after_time));
        assert!(match_obj.is_ready_for_result(after_time));
        assert_eq!(match_obj.time_until_start(after_time), 0);
        assert_eq!(match_obj.time_since_result(after_time), 0);
    }

    #[test]
    fn test_match_predictions_allowed() {
        let env = Env::default();
        let team_a = String::from_str(&env, "Team Alpha");
        let team_b = String::from_str(&env, "Team Beta");
        let match_time = 1640995200;
        
        let match_obj = Match::new(1, 100, team_a, team_b, match_time);

        // 2 hours before match (30 min cutoff) - should allow
        let early_time = match_time - 7200;
        assert!(match_obj.allows_predictions(early_time, 30));

        // 15 minutes before match (30 min cutoff) - should not allow
        let late_time = match_time - 900;
        assert!(!match_obj.allows_predictions(late_time, 30));

        // After match starts - should not allow
        let after_time = match_time + 1800;
        assert!(!match_obj.allows_predictions(after_time, 30));
    }

    #[test]
    fn test_match_validation() {
        let env = Env::default();
        let team_a = String::from_str(&env, "Team Alpha");
        let team_b = String::from_str(&env, "Team Beta");
        let empty_string = String::from_str(&env, "");
        
        // Valid match
        let valid_match = Match::new(1, 100, team_a.clone(), team_b.clone(), 1640995200);
        assert!(valid_match.validate().is_ok());

        // Empty team A name
        let invalid_match1 = Match::new(1, 100, empty_string.clone(), team_b.clone(), 1640995200);
        assert!(invalid_match1.validate().is_err());

        // Empty team B name
        let invalid_match2 = Match::new(1, 100, team_a.clone(), empty_string, 1640995200);
        assert!(invalid_match2.validate().is_err());

        // Same team names
        let invalid_match3 = Match::new(1, 100, team_a.clone(), team_a.clone(), 1640995200);
        assert!(invalid_match3.validate().is_err());

        // Invalid result state - result submitted but no winning team
        let mut invalid_match4 = Match::new(1, 100, team_a.clone(), team_b.clone(), 1640995200);
        invalid_match4.result_submitted = true;
        invalid_match4.winning_team = None;
        assert!(invalid_match4.validate().is_err());

        // Invalid result state - winning team but not submitted
        let mut invalid_match5 = Match::new(1, 100, team_a.clone(), team_b.clone(), 1640995200);
        invalid_match5.result_submitted = false;
        invalid_match5.winning_team = Some(0);
        assert!(invalid_match5.validate().is_err());

        // Invalid winning team value
        let mut invalid_match6 = Match::new(1, 100, team_a.clone(), team_b.clone(), 1640995200);
        invalid_match6.result_submitted = true;
        invalid_match6.winning_team = Some(5); // Invalid value
        invalid_match6.result_timestamp = Some(1640995200);
        assert!(invalid_match6.validate().is_err());
    }