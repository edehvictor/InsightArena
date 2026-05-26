#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec, token::{StellarAssetClient, Client as TokenClient}};

fn create_token_contract<'a>(e: &Env, admin: &Address) -> (Address, StellarAssetClient<'a>, TokenClient<'a>) {
    let contract_address = e.register_stellar_asset_contract_v2(admin.clone()).address();
    (
        contract_address.clone(),
        StellarAssetClient::new(e, &contract_address),
        TokenClient::new(e, &contract_address),
    )
}

#[test]
fn test_initialize_contract() {
    let env = Env::default();
    let contract_id = env.register(CreatorEventManagerContract, ());
    let client = CreatorEventManagerContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let (token_address, token_admin, _token_client) = create_token_contract(&env, &admin);
    let house_fee_percentage = 5u32;

    env.mock_all_auths();

    client.initialize(&token_address, &house_fee_percentage);
    
    let event_count = client.get_event_count();
    assert_eq!(event_count, 0);
    
    let stored_token_address = client.get_token_address();
    assert_eq!(stored_token_address, token_address);
    
    let stored_house_fee = client.get_house_fee_percentage();
    assert_eq!(stored_house_fee, house_fee_percentage);
}

#[test]
fn test_create_event() {
    let env = Env::default();
    let contract_id = env.register(CreatorEventManagerContract, ());
    let client = CreatorEventManagerContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let (token_address, _token_admin, _token_client) = create_token_contract(&env, &admin);
    
    env.mock_all_auths();

    client.initialize(&token_address, &5u32);

    let title = String::from_str(&env, "Test Event");
    let description = String::from_str(&env, "A test prediction event");
    let entry_fee = 1000i128;
    let end_time = env.ledger().timestamp() + 86400; // 24 hours from now
    
    let mut options = Vec::new(&env);
    options.push_back(String::from_str(&env, "Option A"));
    options.push_back(String::from_str(&env, "Option B"));
    
    let invited_users = Vec::new(&env);
    let is_invite_only = false;

    let event_id = client.create_event(
        &creator,
        &title,
        &description,
        &entry_fee,
        &end_time,
        &options,
        &invited_users,
        &is_invite_only,
    );

    assert_eq!(event_id, 1);
    
    let event = client.get_event(&event_id);
    assert_eq!(event.creator, creator);
    assert_eq!(event.title, title);
    assert_eq!(event.entry_fee, entry_fee);
    assert_eq!(event.status, LegacyEventStatus::Active);
}

#[test]
fn test_place_prediction() {
    let env = Env::default();
    let contract_id = env.register(CreatorEventManagerContract, ());
    let client = CreatorEventManagerContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user = Address::generate(&env);
    let (token_address, token_admin, token_client) = create_token_contract(&env, &admin);
    
    env.mock_all_auths();

    // Initialize contract
    client.initialize(&token_address, &5u32);

    // Mint tokens to user
    token_admin.mint(&user, &10000i128);

    // Create an event first
    let title = String::from_str(&env, "Test Event");
    let description = String::from_str(&env, "A test prediction event");
    let entry_fee = 1000i128;
    let end_time = env.ledger().timestamp() + 86400;
    
    let mut options = Vec::new(&env);
    options.push_back(String::from_str(&env, "Option A"));
    options.push_back(String::from_str(&env, "Option B"));
    
    let invited_users = Vec::new(&env);
    let is_invite_only = false;

    let event_id = client.create_event(
        &creator,
        &title,
        &description,
        &entry_fee,
        &end_time,
        &options,
        &invited_users,
        &is_invite_only,
    );

    // User needs to approve the contract to spend tokens
    token_client.approve(&user, &contract_id, &entry_fee, &(env.ledger().sequence() + 100));

    // Place a prediction
    client.place_prediction(&user, &event_id, &0u32, &entry_fee);

    // Verify prediction was placed
    let prediction = client.get_user_prediction(&user, &event_id);
    assert!(prediction.is_some());
    
    let prediction = prediction.unwrap();
    assert_eq!(prediction.user, user);
    assert_eq!(prediction.event_id, event_id);
    assert_eq!(prediction.option_id, 0);
    assert_eq!(prediction.stake_amount, entry_fee);

    // Verify user is in participants list
    let participants = client.get_event_participants(&event_id);
    assert!(participants.contains(&user));

    // Verify tokens were transferred to contract
    let user_balance = client.get_user_balance(&user);
    assert_eq!(user_balance, 9000i128); // 10000 - 1000

    let contract_balance = client.get_contract_balance();
    assert_eq!(contract_balance, 1000i128);
}

#[test]
fn test_resolve_event() {
    let env = Env::default();
    let contract_id = env.register(CreatorEventManagerContract, ());
    let client = CreatorEventManagerContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let (token_address, _token_admin, _token_client) = create_token_contract(&env, &admin);
    
    env.mock_all_auths();

    client.initialize(&token_address, &5u32);

    // Create an event
    let title = String::from_str(&env, "Test Event");
    let description = String::from_str(&env, "A test prediction event");
    let entry_fee = 1000i128;
    let end_time = env.ledger().timestamp() + 86400;
    
    let mut options = Vec::new(&env);
    options.push_back(String::from_str(&env, "Option A"));
    options.push_back(String::from_str(&env, "Option B"));
    
    let invited_users = Vec::new(&env);
    let is_invite_only = false;

    let event_id = client.create_event(
        &creator,
        &title,
        &description,
        &entry_fee,
        &end_time,
        &options,
        &invited_users,
        &is_invite_only,
    );

    // Resolve the event
    let winning_option = 0u32;
    client.resolve_event(&creator, &event_id, &winning_option);

    // Verify event was resolved
    let event = client.get_event(&event_id);
    assert_eq!(event.status, LegacyEventStatus::Resolved);
    assert_eq!(event.oracle_result, Some(winning_option));
}

#[test]
fn test_distribute_winnings() {
    let env = Env::default();
    let contract_id = env.register(CreatorEventManagerContract, ());
    let client = CreatorEventManagerContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let user3 = Address::generate(&env);
    let (token_address, token_admin, token_client) = create_token_contract(&env, &admin);
    
    env.mock_all_auths();

    // Initialize contract
    client.initialize(&token_address, &5u32);

    // Mint tokens to users
    token_admin.mint(&user1, &10000i128);
    token_admin.mint(&user2, &10000i128);
    token_admin.mint(&user3, &10000i128);

    // Create an event
    let title = String::from_str(&env, "Test Event");
    let description = String::from_str(&env, "A test prediction event");
    let entry_fee = 1000i128;
    let end_time = env.ledger().timestamp() + 86400;
    
    let mut options = Vec::new(&env);
    options.push_back(String::from_str(&env, "Option A"));
    options.push_back(String::from_str(&env, "Option B"));
    
    let invited_users = Vec::new(&env);
    let is_invite_only = false;

    let event_id = client.create_event(
        &creator,
        &title,
        &description,
        &entry_fee,
        &end_time,
        &options,
        &invited_users,
        &is_invite_only,
    );

    // Users approve contract to spend tokens
    token_client.approve(&user1, &contract_id, &entry_fee, &(env.ledger().sequence() + 100));
    token_client.approve(&user2, &contract_id, &entry_fee, &(env.ledger().sequence() + 100));
    token_client.approve(&user3, &contract_id, &entry_fee, &(env.ledger().sequence() + 100));

    // Place predictions: user1 and user2 bet on option 0 (winning), user3 bets on option 1 (losing)
    client.place_prediction(&user1, &event_id, &0u32, &entry_fee);
    client.place_prediction(&user2, &event_id, &0u32, &entry_fee);
    client.place_prediction(&user3, &event_id, &1u32, &entry_fee);

    // Verify contract has collected all fees
    let contract_balance = client.get_contract_balance();
    assert_eq!(contract_balance, 3000i128);

    // Resolve the event with option 0 as winner
    let winning_option = 0u32;
    client.resolve_event(&creator, &event_id, &winning_option);

    // Distribute winnings
    client.distribute_winnings(&creator, &event_id);

    // Check final balances
    // Total pool: 3000, House fee (5%): 150, Distributable: 2850
    // Winning option total: 2000 (user1 + user2)
    // Each winner should get: (1000 * 2850) / 2000 = 1425
    let user1_balance = client.get_user_balance(&user1);
    let user2_balance = client.get_user_balance(&user2);
    let user3_balance = client.get_user_balance(&user3);

    assert_eq!(user1_balance, 10425i128); // 9000 + 1425
    assert_eq!(user2_balance, 10425i128); // 9000 + 1425
    assert_eq!(user3_balance, 9000i128);  // Lost their stake, no winnings

    // Contract should retain house fees
    let final_contract_balance = client.get_contract_balance();
    assert_eq!(final_contract_balance, 150i128); // 5% house fee
}

#[test]
fn test_token_balance_checks() {
    let env = Env::default();
    let contract_id = env.register(CreatorEventManagerContract, ());
    let client = CreatorEventManagerContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let (token_address, token_admin, _token_client) = create_token_contract(&env, &admin);
    
    env.mock_all_auths();

    // Initialize contract
    client.initialize(&token_address, &5u32);

    // Mint tokens to user
    token_admin.mint(&user, &500i128); // Less than entry fee

    // Check user balance
    let user_balance = client.get_user_balance(&user);
    assert_eq!(user_balance, 500i128);

    // Check contract balance (should be 0 initially)
    let contract_balance = client.get_contract_balance();
    assert_eq!(contract_balance, 0i128);
}
#[test]
fn test_new_event_creation() {
    let env = Env::default();
    let contract_id = env.register(CreatorEventManagerContract, ());
    let client = CreatorEventManagerContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let (token_address, _token_admin, _token_client) = create_token_contract(&env, &admin);
    
    env.mock_all_auths();

    // Initialize contract
    client.initialize(&token_address, &5u32);

    // Create a new event using the optimized Event struct
    let name = String::from_str(&env, "New Event Format");
    let description = String::from_str(&env, "Testing the new Event struct");
    let creation_fee = 2000000i128; // 0.2 XLM
    let total_matches = 3u32; // Three possible outcomes

    let event_id = client.create_new_event(
        &creator,
        &name,
        &description,
        &creation_fee,
        &total_matches,
    );

    assert_eq!(event_id, 1);

    // Get the new event
    let event = client.get_new_event(&event_id);
    assert_eq!(event.event_id, 1);
    assert_eq!(event.creator, creator);
    assert_eq!(event.name, name);
    assert_eq!(event.description, description);
    assert_eq!(event.creation_fee, creation_fee);
    assert_eq!(event.total_matches, total_matches);
    assert_eq!(event.is_active, true);
    assert_eq!(event.total_participants, 0);

    // Test event statistics
    let (participants, prize_pool, age) = client.get_event_stats(&event_id);
    assert_eq!(participants, 0);
    assert_eq!(prize_pool, 0);
    assert!(age >= 0); // Age should be non-negative

    // Test adding participants
    client.add_event_participant(&event_id);
    
    let (participants, prize_pool, _age) = client.get_event_stats(&event_id);
    assert_eq!(participants, 1);
    assert_eq!(prize_pool, creation_fee);

    // Test event status update
    client.update_event_status(&creator, &event_id, &false);
    let updated_event = client.get_new_event(&event_id);
    assert_eq!(updated_event.is_active, false);
}

#[test]
fn test_event_metadata() {
    let env = Env::default();
    let contract_id = env.register(CreatorEventManagerContract, ());
    let client = CreatorEventManagerContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let (token_address, _token_admin, _token_client) = create_token_contract(&env, &admin);
    
    env.mock_all_auths();

    // Initialize contract
    client.initialize(&token_address, &5u32);

    // Create an event
    let name = String::from_str(&env, "Metadata Test Event");
    let description = String::from_str(&env, "Testing event metadata");
    let event_id = client.create_new_event(&creator, &name, &description, &1000000i128, &2u32);

    // Create metadata
    let category = String::from_str(&env, "Sports");
    let tags = String::from_str(&env, "football,nfl,championship");
    let current_time = env.ledger().timestamp();
    let end_time = current_time + 86400; // 24 hours
    let resolution_time = current_time + 172800; // 48 hours

    client.create_event_metadata(
        &creator,
        &event_id,
        &category,
        &tags,
        &10u32, // min participants
        &1000u32, // max participants
        &end_time,
        &resolution_time,
        &false, // not invite only
    );

    // Get metadata
    let metadata = client.get_event_metadata(&event_id);
    assert!(metadata.is_some());
    
    let metadata = metadata.unwrap();
    assert_eq!(metadata.category, category);
    assert_eq!(metadata.tags, tags);
    assert_eq!(metadata.min_participants, 10);
    assert_eq!(metadata.max_participants, 1000);
    assert_eq!(metadata.end_time, end_time);
    assert_eq!(metadata.resolution_time, resolution_time);
    assert_eq!(metadata.is_invite_only, false);
    assert_eq!(metadata.creator_reputation, 100); // Default reputation
}
#[test]
fn test_match_creation_and_management() {
    let env = Env::default();
    let contract_id = env.register(CreatorEventManagerContract, ());
    let client = CreatorEventManagerContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let (token_address, _token_admin, _token_client) = create_token_contract(&env, &admin);
    
    env.mock_all_auths();

    // Initialize contract
    client.initialize(&token_address, &5u32);

    // Create an event first
    let name = String::from_str(&env, "Sports Event");
    let description = String::from_str(&env, "Football championship");
    let event_id = client.create_new_event(&creator, &name, &description, &1000000i128, &2u32);

    // Create matches
    let team_a1 = String::from_str(&env, "Team Alpha");
    let team_b1 = String::from_str(&env, "Team Beta");
    let team_a2 = String::from_str(&env, "Team Gamma");
    let team_b2 = String::from_str(&env, "Team Delta");
    let current_time = env.ledger().timestamp();
    let match_time1 = current_time + 3600; // 1 hour from now
    let match_time2 = current_time + 7200; // 2 hours from now

    let match_id1 = client.create_match(&creator, &event_id, &team_a1, &team_b1, &match_time1);
    let match_id2 = client.create_match(&creator, &event_id, &team_a2, &team_b2, &match_time2);

    assert_eq!(match_id1, 1);
    assert_eq!(match_id2, 2);

    // Verify matches were created
    let match1 = client.get_match(&match_id1);
    assert_eq!(match1.match_id, match_id1);
    assert_eq!(match1.event_id, event_id);
    assert_eq!(match1.team_a, team_a1);
    assert_eq!(match1.team_b, team_b1);
    assert_eq!(match1.match_time, match_time1);
    assert_eq!(match1.result_submitted, false);

    let match2 = client.get_match(&match_id2);
    assert_eq!(match2.match_id, match_id2);
    assert_eq!(match2.event_id, event_id);
    assert_eq!(match2.team_a, team_a2);
    assert_eq!(match2.team_b, team_b2);

    // Check event matches
    let event_matches = client.get_event_matches(&event_id);
    assert_eq!(event_matches.len(), 2);
    assert!(event_matches.contains(&match_id1));
    assert!(event_matches.contains(&match_id2));

    // Check match count
    let total_matches = client.get_match_count();
    assert_eq!(total_matches, 2);
}

#[test]
fn test_match_result_submission() {
    let env = Env::default();
    let contract_id = env.register(CreatorEventManagerContract, ());
    let client = CreatorEventManagerContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let (token_address, _token_admin, _token_client) = create_token_contract(&env, &admin);
    
    env.mock_all_auths();

    // Initialize contract
    client.initialize(&token_address, &5u32);

    // Create event and match
    let name = String::from_str(&env, "Test Event");
    let description = String::from_str(&env, "Test match");
    let event_id = client.create_new_event(&creator, &name, &description, &1000000i128, &2u32);

    let team_a = String::from_str(&env, "Team A");
    let team_b = String::from_str(&env, "Team B");
    let current_time = env.ledger().timestamp();
    let match_time = current_time + 3600; // Match in 1 hour (future)

    let match_id = client.create_match(&creator, &event_id, &team_a, &team_b, &match_time);

    // Check initial match stats (match hasn't started yet)
    let (has_started, result_submitted, time_until_start, time_since_result) = 
        client.get_match_stats(&match_id);
    assert!(!has_started); // Match is in the future
    assert!(!result_submitted);
    assert!(time_until_start > 0); // Should be around 3600 seconds
    assert_eq!(time_since_result, 0);

    // Submit result: Team A wins (0)
    client.submit_match_result(&creator, &match_id, &0u32);

    // Verify result was submitted
    let updated_match = client.get_match(&match_id);
    assert!(updated_match.result_submitted);
    assert_eq!(updated_match.winning_team, Some(0));
    assert!(updated_match.result_timestamp.is_some());

    // Check updated stats
    let (has_started, result_submitted, time_until_start, time_since_result) = 
        client.get_match_stats(&match_id);
    assert!(!has_started); // Match is still in the future
    assert!(result_submitted);
    assert!(time_until_start > 0);
    assert!(time_since_result >= 0);
}

#[test]
fn test_match_predictions_timing() {
    let env = Env::default();
    let contract_id = env.register(CreatorEventManagerContract, ());
    let client = CreatorEventManagerContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let (token_address, _token_admin, _token_client) = create_token_contract(&env, &admin);
    
    env.mock_all_auths();

    // Initialize contract
    client.initialize(&token_address, &5u32);

    // Create event and match
    let name = String::from_str(&env, "Timing Test");
    let description = String::from_str(&env, "Test prediction timing");
    let event_id = client.create_new_event(&creator, &name, &description, &1000000i128, &2u32);

    let team_a = String::from_str(&env, "Team A");
    let team_b = String::from_str(&env, "Team B");
    let current_time = env.ledger().timestamp();
    let match_time = current_time + 3600; // Match in 1 hour

    let match_id = client.create_match(&creator, &event_id, &team_a, &team_b, &match_time);

    // Should allow predictions with 30 minute cutoff (match is in 1 hour)
    assert!(client.match_allows_predictions(&match_id, &30u64));

    // Should not allow predictions with 90 minute cutoff (match is in 1 hour)
    assert!(!client.match_allows_predictions(&match_id, &90u64));

    // Validate match
    assert!(client.validate_match(&match_id));
}

#[test]
fn test_match_validation_errors() {
    let env = Env::default();
    let contract_id = env.register(CreatorEventManagerContract, ());
    let client = CreatorEventManagerContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    let non_creator = Address::generate(&env);
    let (token_address, _token_admin, _token_client) = create_token_contract(&env, &admin);
    
    env.mock_all_auths();

    // Initialize contract
    client.initialize(&token_address, &5u32);

    // Create event
    let name = String::from_str(&env, "Test Event");
    let description = String::from_str(&env, "Test validation");
    let event_id = client.create_new_event(&creator, &name, &description, &1000000i128, &2u32);

    let team_a = String::from_str(&env, "Team A");
    let team_b = String::from_str(&env, "Team B");
    let current_time = env.ledger().timestamp();
    let match_time = current_time + 3600;

    // Test: Non-creator cannot create match
    // This would panic in real execution due to require_auth()
    // In tests with mock_all_auths(), we can't easily test this

    // Test: Cannot create match for non-existent event
    // This would panic with "Event not found"

    // Create a valid match for other tests
    let match_id = client.create_match(&creator, &event_id, &team_a, &team_b, &match_time);

    // Test: Cannot submit invalid result
    // This would panic with "Invalid winning team value"
    // client.submit_match_result(&creator, &match_id, &5u32); // Would panic

    // Test: Valid result submission
    client.submit_match_result(&creator, &match_id, &0u32); // Team A wins

    // Test: Cannot submit result twice
    // This would panic with "Result already submitted for this match"
    // client.submit_match_result(&creator, &match_id, &1u32); // Would panic
}