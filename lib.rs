#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract]
mod ink_voting_dapp {
    use ink_prelude::vec::Vec;
    use ink_storage::{
        traits::PackedLayout, traits::SpreadAllocate, traits::SpreadLayout, Mapping,
    };

    /// Defines the storage of your contract.
    /// Add new fields to the below struct in order
    /// to add new static storage fields to your contract.
    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct InkVotingDapp {
        elections: Mapping<u32, (AccountId, bool, RegistrationState, ElectionState)>,
        elections_ids: Mapping<Vec<u8>, u32>,
        vote_proposals: Mapping<(u32, u32), u128>,
        proposals_ids: Mapping<(u32, Vec<u8>), u32>,
        proposals_list: Mapping<u32, Vec<Vec<u8>>>,
        voters: Mapping<(u32, AccountId), (u128, bool)>,
        election_nonce: u32,
        election_count: u32,
    }

    #[ink(event)]
    pub struct ElectionCreated {
        name: Vec<u8>,
        id: u32,
        owner: AccountId,
        require_registration: bool,
    }

    #[ink(event)]
    pub struct Voted {
        voter: AccountId,
        proposal: Vec<u8>,
        weight: u128,
    }

    #[ink(event)]
    pub struct Registered {
        voter: AccountId,
        election_id: u32,
    }

    #[ink(event)]
    pub struct OpenRegistration {
        election_id: u32,
        date: Timestamp,
    }

    #[ink(event)]
    pub struct CloseRegistration {
        election_id: u32,
        date: Timestamp,
    }

    #[ink(event)]
    pub struct OpenElection {
        election_id: u32,
        date: Timestamp,
    }

    #[ink(event)]
    pub struct CloseElection {
        election_id: u32,
        date: Timestamp,
    }

    #[ink(event)]
    pub struct ChangeOwnership {
        election_id: u32,
        new_owner: AccountId,
    }

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        InsufficientProposals,
        ElectionNotValid,
        VoterNotRegistred,
        VoterHasNotSoMuchWeight,
        VoterHasAlreadyVoted,
        InvalidProposal,
        OnlyOwner,
        ElectionClosed,
        RegistrationClosed,
        VoterAlreadyRegistered,
    }
    #[derive(SpreadLayout, PackedLayout, Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum RegistrationState {
        RegistrationOpen,
        RegistrationClosed,
    }
    impl Default for RegistrationState {
        fn default() -> Self {
            RegistrationState::RegistrationClosed
        }
    }
    #[derive(SpreadLayout, PackedLayout, Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum ElectionState {
        ElectionOpen,
        ElectionClosed,
    }
    impl Default for ElectionState {
        fn default() -> Self {
            ElectionState::ElectionClosed
        }
    }
    pub type Result<T> = core::result::Result<T, Error>;

    impl InkVotingDapp {
        #[ink(constructor)]
        pub fn new() -> Self {
            ink_lang::utils::initialize_contract(|contract| Self::new_init(contract))
        }

        fn new_init(&mut self) {
            self.election_nonce = 1;
            self.election_count = 0;
        }

        #[ink(message)]
        pub fn create_election(
            &mut self,
            name: Vec<u8>,
            required_registration: bool,
            proposals: Vec<Vec<u8>>,
        ) -> Result<()> {
            self.check_existence(&name)?;
            self.check_sufficient_proposals(&proposals)?;
            let election_id = self.election_nonce;
            self.insert_election(
                &name,
                &election_id,
                Self::env().caller(),
                required_registration,
                &proposals,
            );
            Self::env().emit_event(ElectionCreated {
                name: name,
                id: election_id,
                owner: Self::env().caller(),
                require_registration: required_registration,
            });
            self.election_nonce += 1;
            self.election_count += 1;
            Ok(())
        }
        #[ink(message)]
        pub fn vote(&mut self, name: Vec<u8>, proposal: Vec<u8>, weight: u128) -> Result<()> {
            self.check_existence(&name)?;
            self.check_election_open(&name)?;
            let election_id = self.elections_ids.get(&name).unwrap();
            let voter_address = Self::env().caller();
            self.check_if_registration_needed(&election_id, &voter_address)?;
            self.check_voter_can_vote(&election_id, &voter_address, &weight)?;
            self.check_proposal_valid(&election_id, &proposal)?;
            self._vote(&election_id, &proposal, &voter_address, &weight);
            Self::env().emit_event(Voted {
                voter: voter_address,
                proposal: proposal,
                weight: weight,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn register_me(&mut self, name: Vec<u8>) -> Result<()> {
            self.register(name, Self::env().caller())?;
            Ok(())
        }

        #[ink(message)]
        pub fn register(&mut self, name: Vec<u8>, voter: AccountId) -> Result<()> {
            self.check_existence(&name)?;
            self.check_registration_open(&name)?;
            let election_id = self.elections_ids.get(&name).unwrap();
            if self.is_voter_registered(&election_id, &voter) {
                return Err(Error::VoterAlreadyRegistered);
            };
            self.register_voter(&voter, &election_id);
            Self::env().emit_event(Registered {
                voter: voter,
                election_id: election_id,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn delegate_vote(
            &mut self,
            name: Vec<u8>,
            delegate: AccountId,
            weight: u128,
        ) -> Result<()> {
            self.check_existence(&name)?;
            let election_id = self.elections_ids.get(&name).unwrap();
            let delegator_address = Self::env().caller();
            self.check_if_registration_needed(&election_id, &delegator_address)?;
            self.check_if_registration_needed(&election_id, &delegate)?;
            self.check_voter_can_vote(&election_id, &delegator_address, &weight)?;
            self.delegate(&election_id, &delegate, &delegator_address, &weight);
            Ok(())
        }

        fn subtract_weight(&mut self, election_id: &u32, voter: &AccountId, weight: &u128) {
            let voter_weight = self.voters.get((election_id, voter)).unwrap().0;
            if voter_weight - weight == 0 {
                self.voters.insert((&election_id, &voter), &(0, true));
            } else {
                self.voters
                    .insert((&election_id, &voter), &(voter_weight - weight, false));
            }
        }

        fn delegate(
            &mut self,
            election_id: &u32,
            delegate: &AccountId,
            delegator_address: &AccountId,
            weight: &u128,
        ) {
            let weight_delegate = &self.voters.get((election_id, delegate)).unwrap().0;
            self.voters
                .insert((election_id, delegate), &(weight_delegate + weight, false));
            self.subtract_weight(election_id, delegator_address, weight);
        }

        #[ink(message)]
        pub fn open_registration(&mut self, name: Vec<u8>) -> Result<()> {
            self.check_existence(&name)?;
            let election_id = self.elections_ids.get(&name).unwrap();
            self.only_owner(&election_id, &Self::env().caller())?;
            self.elections.get(election_id).unwrap().2 = RegistrationState::RegistrationOpen;
            Self::env().emit_event(OpenRegistration {
                election_id: election_id,
                date: Self::env().block_timestamp(),
            });
            Ok(())
        }

        #[ink(message)]
        pub fn close_registration(&mut self, name: Vec<u8>) -> Result<()> {
            self.check_existence(&name)?;
            let election_id = self.elections_ids.get(&name).unwrap();
            self.only_owner(&election_id, &Self::env().caller())?;
            self.elections.get(election_id).unwrap().2 = RegistrationState::RegistrationClosed;
            Self::env().emit_event(CloseRegistration {
                election_id: election_id,
                date: Self::env().block_timestamp(),
            });
            Ok(())
        }

        #[ink(message)]
        pub fn open_election(&mut self, name: Vec<u8>) -> Result<()> {
            self.check_existence(&name)?;
            let election_id = self.elections_ids.get(&name).unwrap();
            self.only_owner(&election_id, &Self::env().caller())?;
            self.elections.get(election_id).unwrap().3 = ElectionState::ElectionOpen;
            Self::env().emit_event(OpenElection {
                election_id: election_id,
                date: Self::env().block_timestamp(),
            });
            Ok(())
        }

        #[ink(message)]
        pub fn close_election(&mut self, name: Vec<u8>) -> Result<()> {
            self.check_existence(&name)?;
            let election_id = self.elections_ids.get(&name).unwrap();
            self.only_owner(&election_id, &Self::env().caller())?;
            self.elections.get(election_id).unwrap().3 = ElectionState::ElectionClosed;
            Self::env().emit_event(CloseElection {
                election_id: election_id,
                date: Self::env().block_timestamp(),
            });
            Ok(())
        }

        #[ink(message)]
        pub fn change_ownership(&mut self, name: Vec<u8>, new_owner: AccountId) -> Result<()> {
            self.check_existence(&name)?;
            let election_id = self.elections_ids.get(&name).unwrap();
            self.only_owner(&election_id, &Self::env().caller())?;
            self.elections.get(election_id).unwrap().0 = new_owner;
            Self::env().emit_event(ChangeOwnership {
                election_id: election_id,
                new_owner: new_owner,
            });
            Ok(())
        }

        fn register_voter(&mut self, voter: &AccountId, election_id: &u32) {
            self.voters.insert((election_id, voter), &(1, false));
        }
        #[ink(message)]
        pub fn get_number_elections(&self) -> u32 {
            self.election_count
        }

        #[ink(message)]
        pub fn get_proposal_for_election(&self, name: Vec<u8>) -> Vec<Vec<u8>> {
            let election_id = self.elections_ids.get(&name).unwrap_or_default();
            self.proposals_list.get(election_id).unwrap_or_default()
        }

        #[ink(message)]
        pub fn get_owner_of_election(&self, election_id: u32) -> AccountId {
            self.elections.get(election_id).unwrap_or_default().0
        }

        #[ink(message)]
        pub fn election_exists(&self, name: Vec<u8>) -> bool {
            self._election_exists(&name)
        }

        fn _election_exists(&self, name: &Vec<u8>) -> bool {
            self.elections_ids.get(name) != None
        }

        fn _is_election_open(&self, name: &Vec<u8>) -> bool {
            let election_id = &self.elections_ids.get(name).unwrap_or_default();
            self.elections.get(election_id).unwrap().3 == ElectionState::ElectionOpen
        }

        fn _is_registration_open(&self, name: &Vec<u8>) -> bool {
            let election_id = &self.elections_ids.get(name).unwrap_or_default();
            self.elections.get(election_id).unwrap().2 == RegistrationState::RegistrationOpen
        }

        #[ink(message)]
        pub fn is_election_open(&self, name: Vec<u8>) -> bool {
            self._is_election_open(&name)
        }

        #[ink(message)]
        pub fn is_registration_open(&self, name: Vec<u8>) -> bool {
            self._is_registration_open(&name)
        }

        fn is_owner(&self, account: &AccountId, election_id: &u32) -> bool {
            account == &self.elections.get(election_id).unwrap_or_default().0
        }
        fn check_existence(&self, name: &Vec<u8>) -> Result<()> {
            if self._election_exists(name) {
                Err(Error::ElectionNotValid)
            } else {
                Ok(())
            }
        }
        fn check_sufficient_proposals(&self, proposals: &Vec<Vec<u8>>) -> Result<()> {
            if proposals.len() == 0 {
                Err(Error::InsufficientProposals)
            } else {
                Ok(())
            }
        }
        fn insert_proposal(&mut self, election_id: &u32, proposal: &Vec<u8>, proposal_id: &u32) {
            self.vote_proposals
                .insert((election_id, (proposal_id + 1)), &0);
            self.proposals_ids
                .insert((election_id, proposal), &(proposal_id + 1));
        }
        fn insert_election(
            &mut self,
            name: &Vec<u8>,
            election_id: &u32,
            owner: AccountId,
            required_registration: bool,
            proposals: &Vec<Vec<u8>>,
        ) {
            self.elections.insert(
                &election_id,
                &(
                    owner,
                    required_registration,
                    RegistrationState::RegistrationClosed,
                    ElectionState::ElectionClosed,
                ),
            );
            self.elections_ids.insert(name, election_id);
            for i in 0..proposals.len() {
                self.insert_proposal(&election_id, proposals.get(i).unwrap(), &(i as u32));
            }
            self.proposals_list.insert(&election_id, proposals);
        }
        fn check_election_open(&self, name: &Vec<u8>) -> Result<()> {
            if !self._is_election_open(name) {
                Err(Error::ElectionClosed)
            } else {
                Ok(())
            }
        }
        fn check_registration_open(&self, name: &Vec<u8>) -> Result<()> {
            if !self._is_registration_open(name) {
                Err(Error::RegistrationClosed)
            } else {
                Ok(())
            }
        }
        fn is_voter_registered(&self, election_id: &u32, voter_address: &AccountId) -> bool {
            !(self.voters.get((election_id, voter_address)) == None)
        }

        fn check_voter_registered(
            &self,
            election_id: &u32,
            voter_address: &AccountId,
        ) -> Result<()> {
            if !self.is_voter_registered(election_id, voter_address) {
                Err(Error::VoterNotRegistred)
            } else {
                Ok(())
            }
        }
        fn check_proposal_valid(&self, election_id: &u32, proposal: &Vec<u8>) -> Result<()> {
            if self.proposals_ids.get((election_id, proposal)) == None {
                Err(Error::InvalidProposal)
            } else {
                Ok(())
            }
        }
        fn check_voter_can_vote(
            &self,
            election_id: &u32,
            voter_address: &AccountId,
            weight: &u128,
        ) -> Result<()> {
            let (voter_weight, voter_has_voted) =
                self.voters.get((election_id, voter_address)).unwrap();
            if voter_has_voted {
                Err(Error::VoterHasAlreadyVoted)
            } else if &voter_weight < weight {
                Err(Error::VoterHasNotSoMuchWeight)
            } else {
                Ok(())
            }
        }
        fn _vote(
            &mut self,
            election_id: &u32,
            proposal: &Vec<u8>,
            voter_address: &AccountId,
            weight: &u128,
        ) {
            let proposal_id = self.proposals_ids.get((election_id, proposal)).unwrap();
            let vote_proposal = self.vote_proposals.get((election_id, proposal_id)).unwrap();
            self.vote_proposals
                .insert((election_id, proposal_id), &(vote_proposal + weight));
            self.subtract_weight(election_id, voter_address, weight);
        }
        fn only_owner(&self, election_id: &u32, address: &AccountId) -> Result<()> {
            if !self.is_owner(address, election_id) {
                Err(Error::OnlyOwner)
            } else {
                Ok(())
            }
        }
        fn check_if_registration_needed(
            &mut self,
            election_id: &u32,
            voter_address: &AccountId,
        ) -> Result<()> {
            if self.elections.get(&election_id).unwrap().1 {
                self.check_voter_registered(&election_id, &voter_address)?;
            } else {
                if !self.is_voter_registered(&election_id, &voter_address) {
                    self.register_voter(&voter_address, &election_id);
                }
            }
            Ok(())
        }
    }

    /// Unit tests in Rust are normally defined within such a `#[cfg(test)]`
    /// module and test functions are marked with a `#[test]` attribute.
    /// The below code is technically just normal Rust code.
    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;

        /// Imports `ink_lang` so we can use `#[ink::test]`.
        use ink_lang as ink;

        /// We test if the default constructor does its job.
        #[ink::test]
        fn new_works() {
            let ink_voting_dapp = InkVotingDapp::new();
            assert_eq!(ink_voting_dapp.get_number_elections(), 0);
        }

        fn to_ut8(string: &str) -> Vec<u8> {
            string.as_bytes().to_vec()
        }

        /// We test a simple use case of our contract.
        #[ink::test]
        fn create_election_works() {
            let mut ink_voting_dapp = InkVotingDapp::new();
            //let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>();
            assert_eq!(
                ink_voting_dapp.create_election(
                    to_ut8("firstelection"),
                    false,
                    vec![to_ut8("firstproposal"), to_ut8("secondproposal")]
                ),
                Ok(())
            );
            assert_eq!(
                ink_voting_dapp.create_election(
                    to_ut8("firstelection"),
                    false,
                    vec![to_ut8("firstproposal")]
                ),
                Err(Error::ElectionNotValid)
            );
            assert_eq!(
                ink_voting_dapp.create_election(to_ut8("secondelection"), false, vec![]),
                Err(Error::InsufficientProposals)
            );
            assert_eq!(ink_voting_dapp.get_number_elections(), 1);
            assert_eq!(ink_env::test::recorded_events().count(), 1);
            assert_eq!(
                ink_voting_dapp.get_proposal_for_election(to_ut8("firstelection")),
                vec![to_ut8("firstproposal"), to_ut8("secondproposal")]
            );
            assert_eq!(
                ink_voting_dapp.get_owner_of_election(1),
                ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().alice
            );
        }
    }
}
