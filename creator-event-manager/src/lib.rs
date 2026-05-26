#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Vec};

mod token;
mod storage_types;

use token::TokenHelper;
use storage_types::{Event, EventMetadata, Match, MatchResult};

/// Event status enumeration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegacyEventStatus {
    Active,
    Resolved,
    Cancelled,
}

/// Prediction option structure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredictionOption {
    pub id: u32,
    pub description: String,
    pub total_stake: i128,
}

/// Legacy Event structure - replaced by storage_types::Event
/// Keeping for backward compatibility during migration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyEvent {
    pub id: u64,
    pub creator: Address,
    pub title: String,
    pub description: String,
    pub entry_fee: i128,
    pub status: LegacyEventStatus,
    pub end_time: u64,
    pub options: Vec<PredictionOption>,
    pub invited_users: Vec<Address>,
    pub is_invite_only: bool,
    pub oracle_result: Option<u32>,
}

/// User prediction structure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserPrediction {
    pub user: Address,
    pub event_id: u64,
    pub option_id: u32,
    pub stake_amount: i128,
    pub timestamp: u64,
}

/// Storage keys for the contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    EventCounter,
    Event(u64),
    EventMetadata(u64),
    Match(u64), // match_id
    EventMatches(u64), // event_id -> Vec<u64> (match_ids)
    MatchCounter,
    UserPrediction(Address, u64),
    EventParticipants(u64),
    TokenAddress,
    Treasury,
    HouseFeePercentage,
}

/// Main contract structure
#[contract]
pub struct CreatorEventManagerContract;

#[contractimpl]
impl CreatorEventManagerContract {
    /// Initialize the contract with token address and configuration
    pub fn initialize(env: Env, token_address: Address, house_fee_percentage: u32) {
        // Validate inputs
        assert!(house_fee_percentage <= 20, "House fee cannot exceed 20%");
        
        // Validate token address
        TokenHelper::validate_token_address(&token_address).expect("Invalid token address");
        
        // Set initial values
        env.storage().instance().set(&DataKey::EventCounter, &0u64);
        env.storage().instance().set(&DataKey::MatchCounter, &0u64);
        env.storage().instance().set(&DataKey::TokenAddress, &token_address);
        env.storage().instance().set(&DataKey::HouseFeePercentage, &house_fee_percentage);
        env.storage().instance().set(&DataKey::Treasury, &env.current_contract_address());
    }

    /// Create a new prediction event
    pub fn create_event(
        env: Env,
        creator: Address,
        title: String,
        description: String,
        entry_fee: i128,
        end_time: u64,
        options: Vec<String>,
        invited_users: Vec<Address>,
        is_invite_only: bool,
    ) -> u64 {
        // Authenticate creator
        creator.require_auth();

        // Get and increment event counter
        let mut event_counter: u64 = env.storage()
            .instance()
            .get(&DataKey::EventCounter)
            .unwrap_or(0);
        
        event_counter += 1;
        env.storage().instance().set(&DataKey::EventCounter, &event_counter);

        // Create prediction options
        let mut prediction_options = Vec::new(&env);
        for (i, option_desc) in options.iter().enumerate() {
            prediction_options.push_back(PredictionOption {
                id: i as u32,
                description: option_desc,
                total_stake: 0,
            });
        }

        // Create the event (legacy format for backward compatibility)
        let event = LegacyEvent {
            id: event_counter,
            creator: creator.clone(),
            title,
            description,
            entry_fee,
            status: LegacyEventStatus::Active,
            end_time,
            options: prediction_options,
            invited_users,
            is_invite_only,
            oracle_result: None,
        };

        // Store the event
        env.storage().persistent().set(&DataKey::Event(event_counter), &event);

        // Initialize empty participants list
        let participants: Vec<Address> = Vec::new(&env);
        env.storage().persistent().set(&DataKey::EventParticipants(event_counter), &participants);

        event_counter
    }

    /// Place a prediction on an event
    pub fn place_prediction(
        env: Env,
        user: Address,
        event_id: u64,
        option_id: u32,
        stake_amount: i128,
    ) {
        // Authenticate user
        user.require_auth();

        // Get the event
        let mut event: LegacyEvent = env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .expect("Event not found");

        // Validate event is active
        assert_eq!(event.status, LegacyEventStatus::Active, "Event is not active");

        // Check if event has ended
        let current_time = env.ledger().timestamp();
        assert!(current_time < event.end_time, "Event has ended");

        // Check invite-only access
        if event.is_invite_only {
            assert!(
                event.invited_users.contains(&user),
                "User not invited to this event"
            );
        }

        // Validate option exists
        assert!(
            (option_id as usize) < event.options.len().try_into().unwrap(),
            "Invalid option ID"
        );

        // Validate stake amount matches entry fee
        assert_eq!(stake_amount, event.entry_fee, "Stake amount must match entry fee");

        // Check if user already has a prediction for this event
        let existing_prediction = env.storage()
            .persistent()
            .get::<DataKey, UserPrediction>(&DataKey::UserPrediction(user.clone(), event_id));
        
        assert!(existing_prediction.is_none(), "User already has a prediction for this event");

        // Get token address
        let token_address: Address = env.storage()
            .instance()
            .get(&DataKey::TokenAddress)
            .expect("Token address not set");

        // Collect entry fee from user
        TokenHelper::collect_entry_fee(&env, &token_address, &user, stake_amount)
            .expect("Failed to collect entry fee");

        // Update option total stake
        let mut updated_options = event.options.clone();
        let mut option = updated_options.get(option_id as u32).unwrap();
        option.total_stake += stake_amount;
        updated_options.set(option_id as u32, option);
        event.options = updated_options;

        // Store updated event
        env.storage().persistent().set(&DataKey::Event(event_id), &event);

        // Create and store user prediction
        let prediction = UserPrediction {
            user: user.clone(),
            event_id,
            option_id,
            stake_amount,
            timestamp: current_time,
        };
        env.storage().persistent().set(&DataKey::UserPrediction(user.clone(), event_id), &prediction);

        // Add user to participants list
        let mut participants: Vec<Address> = env.storage()
            .persistent()
            .get(&DataKey::EventParticipants(event_id))
            .unwrap_or(Vec::new(&env));
        
        if !participants.contains(&user) {
            participants.push_back(user);
            env.storage().persistent().set(&DataKey::EventParticipants(event_id), &participants);
        }
    }

    /// Resolve an event with oracle result (only creator can call)
    pub fn resolve_event(env: Env, creator: Address, event_id: u64, winning_option_id: u32) {
        // Authenticate creator
        creator.require_auth();

        // Get the event
        let mut event: LegacyEvent = env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .expect("Event not found");

        // Verify caller is the event creator
        assert_eq!(event.creator, creator, "Only event creator can resolve");

        // Validate event is active
        assert_eq!(event.status, LegacyEventStatus::Active, "Event is not active");

        // Validate winning option exists
        assert!(
            (winning_option_id as usize) < event.options.len().try_into().unwrap(),
            "Invalid winning option ID"
        );

        // Update event status and oracle result
        event.status = LegacyEventStatus::Resolved;
        event.oracle_result = Some(winning_option_id);

        // Store updated event
        env.storage().persistent().set(&DataKey::Event(event_id), &event);
    }

    /// Get event details (legacy format)
    pub fn get_event(env: Env, event_id: u64) -> LegacyEvent {
        env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .expect("Event not found")
    }

    /// Get user's prediction for an event
    pub fn get_user_prediction(env: Env, user: Address, event_id: u64) -> Option<UserPrediction> {
        env.storage()
            .persistent()
            .get(&DataKey::UserPrediction(user, event_id))
    }

    /// Get total number of events created
    pub fn get_event_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::EventCounter)
            .unwrap_or(0)
    }

    /// Get participants of an event
    pub fn get_event_participants(env: Env, event_id: u64) -> Vec<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::EventParticipants(event_id))
            .unwrap_or(Vec::new(&env))
    }

    /// Distribute winnings to participants after event resolution
    pub fn distribute_winnings(env: Env, creator: Address, event_id: u64) {
        // Authenticate creator
        creator.require_auth();

        // Get the event
        let event: LegacyEvent = env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .expect("Event not found");

        // Verify caller is the event creator
        assert_eq!(event.creator, creator, "Only event creator can distribute winnings");

        // Validate event is resolved
        assert_eq!(event.status, LegacyEventStatus::Resolved, "Event is not resolved");
        
        let winning_option_id = event.oracle_result.expect("No oracle result available");

        // Get token address and house fee
        let token_address: Address = env.storage()
            .instance()
            .get(&DataKey::TokenAddress)
            .expect("Token address not set");
        
        let house_fee_percentage: u32 = env.storage()
            .instance()
            .get(&DataKey::HouseFeePercentage)
            .unwrap_or(5);

        // Calculate total pool and winning option total
        let mut option_stakes = Vec::new(&env);
        for option in event.options.iter() {
            option_stakes.push_back(option.total_stake);
        }
        let total_pool = TokenHelper::calculate_total_pool(&option_stakes);
        let winning_option_total = event.options.get(winning_option_id).unwrap().total_stake;

        // Get all participants
        let participants = Self::get_event_participants(env.clone(), event_id);

        // Distribute winnings to each participant who bet on the winning option
        for participant in participants.iter() {
            if let Some(prediction) = env.storage()
                .persistent()
                .get::<DataKey, UserPrediction>(&DataKey::UserPrediction(participant.clone(), event_id))
            {
                if prediction.option_id == winning_option_id {
                    let winnings = TokenHelper::calculate_winnings(
                        prediction.stake_amount,
                        winning_option_total,
                        total_pool,
                        house_fee_percentage,
                    );

                    if winnings > 0 {
                        TokenHelper::distribute_winnings(&env, &token_address, &participant, winnings)
                            .expect("Failed to distribute winnings");
                    }
                }
            }
        }
    }

    /// Get contract's token balance
    pub fn get_contract_balance(env: Env) -> i128 {
        let token_address: Address = env.storage()
            .instance()
            .get(&DataKey::TokenAddress)
            .expect("Token address not set");
        
        TokenHelper::get_contract_balance(&env, &token_address)
    }

    /// Get user's token balance
    pub fn get_user_balance(env: Env, user: Address) -> i128 {
        let token_address: Address = env.storage()
            .instance()
            .get(&DataKey::TokenAddress)
            .expect("Token address not set");
        
        TokenHelper::get_balance(&env, &token_address, &user)
    }

    /// Withdraw house fees (only contract admin can call)
    pub fn withdraw_house_fees(env: Env, admin: Address, amount: i128) {
        // Authenticate admin
        admin.require_auth();

        // Get token address
        let token_address: Address = env.storage()
            .instance()
            .get(&DataKey::TokenAddress)
            .expect("Token address not set");

        // Transfer house fees to admin
        TokenHelper::transfer(&env, &token_address, &admin, amount)
            .expect("Failed to withdraw house fees");
    }

    /// Get the configured token address
    pub fn get_token_address(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::TokenAddress)
            .expect("Token address not set")
    }

    /// Get the house fee percentage
    pub fn get_house_fee_percentage(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::HouseFeePercentage)
            .unwrap_or(5)
    }

    /// Create a new event using the optimized Event struct
    pub fn create_new_event(
        env: Env,
        creator: Address,
        name: String,
        description: String,
        creation_fee: i128,
        total_matches: u32,
    ) -> u64 {
        // Authenticate creator
        creator.require_auth();

        // Get and increment event counter
        let mut event_counter: u64 = env.storage()
            .instance()
            .get(&DataKey::EventCounter)
            .unwrap_or(0);
        
        event_counter += 1;
        env.storage().instance().set(&DataKey::EventCounter, &event_counter);

        // Create the new event
        let current_time = env.ledger().timestamp();
        let event = Event::new(
            event_counter,
            creator,
            name,
            description,
            creation_fee,
            current_time,
            total_matches,
        );

        // Store the event
        env.storage().persistent().set(&DataKey::Event(event_counter), &event);

        event_counter
    }

    /// Get event using the new Event struct
    pub fn get_new_event(env: Env, event_id: u64) -> Event {
        env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .expect("Event not found")
    }

    /// Update event status
    pub fn update_event_status(env: Env, creator: Address, event_id: u64, is_active: bool) {
        // Authenticate creator
        creator.require_auth();

        // Get the event
        let mut event: Event = env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .expect("Event not found");

        // Verify caller is the event creator
        assert_eq!(event.creator, creator, "Only event creator can update status");

        // Update status
        if is_active {
            // Reactivate if possible
            if !event.can_accept_predictions() {
                panic!("Event cannot be reactivated");
            }
        } else {
            event.deactivate();
        }

        // Store updated event
        env.storage().persistent().set(&DataKey::Event(event_id), &event);
    }

    /// Add participant to event (used when placing predictions)
    pub fn add_event_participant(env: Env, event_id: u64) {
        // Get the event
        let mut event: Event = env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .expect("Event not found");

        // Add participant
        event.add_participant().expect("Failed to add participant");

        // Store updated event
        env.storage().persistent().set(&DataKey::Event(event_id), &event);
    }

    /// Get event statistics
    pub fn get_event_stats(env: Env, event_id: u64) -> (u32, i128, u64) {
        let event: Event = env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .expect("Event not found");

        (
            event.total_participants,
            event.get_total_prize_pool(),
            event.get_age_seconds(env.ledger().timestamp()),
        )
    }

    /// Create event metadata
    pub fn create_event_metadata(
        env: Env,
        creator: Address,
        event_id: u64,
        category: String,
        tags: String,
        min_participants: u32,
        max_participants: u32,
        end_time: u64,
        resolution_time: u64,
        is_invite_only: bool,
    ) {
        // Authenticate creator
        creator.require_auth();

        // Verify event exists and creator owns it
        let event: Event = env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .expect("Event not found");
        
        assert_eq!(event.creator, creator, "Only event creator can set metadata");

        // Get creator reputation (placeholder - would integrate with reputation system)
        let creator_reputation = 100u32; // Default reputation

        // Create metadata
        let metadata = EventMetadata::new(
            category,
            tags,
            min_participants,
            max_participants,
            end_time,
            resolution_time,
            is_invite_only,
            creator_reputation,
        );

        // Store metadata with a separate key
        env.storage().persistent().set(&DataKey::EventMetadata(event_id), &metadata);
    }

    /// Get event metadata
    pub fn get_event_metadata(env: Env, event_id: u64) -> Option<EventMetadata> {
        env.storage()
            .persistent()
            .get(&DataKey::EventMetadata(event_id))
    }

    /// Create a new match within an event
    pub fn create_match(
        env: Env,
        creator: Address,
        event_id: u64,
        team_a: String,
        team_b: String,
        match_time: u64,
    ) -> u64 {
        // Authenticate creator
        creator.require_auth();

        // Verify event exists and creator owns it
        let event: Event = env.storage()
            .persistent()
            .get(&DataKey::Event(event_id))
            .expect("Event not found");
        
        assert_eq!(event.creator, creator, "Only event creator can create matches");
        assert!(event.is_active, "Cannot create matches for inactive events");

        // Get and increment match counter
        let mut match_counter: u64 = env.storage()
            .instance()
            .get(&DataKey::MatchCounter)
            .unwrap_or(0);
        
        match_counter += 1;
        env.storage().instance().set(&DataKey::MatchCounter, &match_counter);

        // Create the match
        let match_obj = Match::new(
            match_counter,
            event_id,
            team_a,
            team_b,
            match_time,
        );

        // Validate match data
        match_obj.validate().expect("Invalid match data");

        // Store the match
        env.storage().persistent().set(&DataKey::Match(match_counter), &match_obj);

        // Add match to event's match list
        let mut event_matches: Vec<u64> = env.storage()
            .persistent()
            .get(&DataKey::EventMatches(event_id))
            .unwrap_or(Vec::new(&env));
        
        event_matches.push_back(match_counter);
        env.storage().persistent().set(&DataKey::EventMatches(event_id), &event_matches);

        match_counter
    }

    /// Get match details
    pub fn get_match(env: Env, match_id: u64) -> Match {
        env.storage()
            .persistent()
            .get(&DataKey::Match(match_id))
            .expect("Match not found")
    }

    /// Get all matches for an event
    pub fn get_event_matches(env: Env, event_id: u64) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::EventMatches(event_id))
            .unwrap_or(Vec::new(&env))
    }

    /// Submit result for a match
    pub fn submit_match_result(
        env: Env,
        creator: Address,
        match_id: u64,
        winning_team: u32,
    ) {
        // Authenticate creator
        creator.require_auth();

        // Get the match
        let mut match_obj: Match = env.storage()
            .persistent()
            .get(&DataKey::Match(match_id))
            .expect("Match not found");

        // Verify event creator owns this match
        let event: Event = env.storage()
            .persistent()
            .get(&DataKey::Event(match_obj.event_id))
            .expect("Event not found");
        
        assert_eq!(event.creator, creator, "Only event creator can submit results");

        // Validate and convert result
        let result = MatchResult::from_u32(winning_team)
            .expect("Invalid winning team value (must be 0, 1, or 2)");

        // Submit the result
        let current_time = env.ledger().timestamp();
        match_obj.submit_result(result, current_time)
            .expect("Failed to submit result");

        // Store updated match
        env.storage().persistent().set(&DataKey::Match(match_id), &match_obj);
    }

    /// Check if match allows predictions at current time
    pub fn match_allows_predictions(env: Env, match_id: u64, cutoff_minutes: u64) -> bool {
        let match_obj: Match = env.storage()
            .persistent()
            .get(&DataKey::Match(match_id))
            .expect("Match not found");

        let current_time = env.ledger().timestamp();
        match_obj.allows_predictions(current_time, cutoff_minutes)
    }

    /// Get match statistics
    pub fn get_match_stats(env: Env, match_id: u64) -> (bool, bool, u64, u64) {
        let match_obj: Match = env.storage()
            .persistent()
            .get(&DataKey::Match(match_id))
            .expect("Match not found");

        let current_time = env.ledger().timestamp();
        
        (
            match_obj.has_started(current_time),
            match_obj.result_submitted,
            match_obj.time_until_start(current_time),
            match_obj.time_since_result(current_time),
        )
    }

    /// Get total number of matches created
    pub fn get_match_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::MatchCounter)
            .unwrap_or(0)
    }

    /// Validate match data
    pub fn validate_match(env: Env, match_id: u64) -> bool {
        let match_obj: Match = env.storage()
            .persistent()
            .get(&DataKey::Match(match_id))
            .expect("Match not found");

        match_obj.validate().is_ok()
    }
}

mod test;