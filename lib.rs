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
        proposals_ids: Mapping<Vec<u8>, u32>,
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

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        InsufficientProposals,
        DoubleElection,
        InesistentElection,
        VoterNotRegistred,
        VoterNotHasSoMuchWeight,
        VoterHasAlreadyVoted,
        InvalidProposal,
        OnlyOwner,
    }
    #[derive(SpreadLayout, PackedLayout, Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum RegistrationState {
        RegistationOpen,
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
            _name: Vec<u8>,
            _required_registration: bool,
            _proposals: Vec<Vec<u8>>,
        ) -> Result<()> {
            if _proposals.len() == 0 {
                return Err(Error::InsufficientProposals);
            }
            if self.election_exists(&_name) {
                return Err(Error::DoubleElection);
            }
            let election_id = self.election_nonce;
            let owner = Self::env().caller();
            self.elections.insert(
                &election_id,
                &(
                    owner,
                    _required_registration,
                    RegistrationState::RegistrationClosed,
                    ElectionState::ElectionClosed,
                ),
            );
            self.elections_ids.insert(&_name, &election_id);
            for i in 0.._proposals.len() {
                self.vote_proposals
                    .insert((&election_id, &((i + 1) as u32)), &0);
                self.proposals_ids
                    .insert(&_proposals.get(i).unwrap(), &((i + 1) as u32))
            }
            self.proposals_list.insert(&election_id, &_proposals);
            Self::env().emit_event(ElectionCreated {
                name: _name,
                id: election_id,
                owner: owner,
                require_registration: _required_registration,
            });
            self.election_nonce += 1;
            self.election_count += 1;
            Ok(())
        }

        /// Simply returns the current value of our `bool`.
        #[ink(message)]
        pub fn vote(&mut self, _name: Vec<u8>, _proposal: Vec<u8>, _weight: u128) -> Result<()> {
            if !self.election_exists(&_name) {
                return Err(Error::InesistentElection);
            }
            let election_id = self.elections_ids.get(&_name).unwrap();
            let require_registration = self.elections.get(&election_id).unwrap().1;
            let voter_address = Self::env().caller();
            if require_registration {
                if self.voters.get((&election_id, &voter_address)) == None {
                    return Err(Error::VoterNotRegistred);
                }
            } else {
                let voter = (1, false);
                self.voters.insert((&election_id, &voter_address), &voter)
            }
            let (voter_weight, voter_has_voted) =
                self.voters.get((&election_id, &voter_address)).unwrap();
            if voter_has_voted {
                return Err(Error::VoterHasAlreadyVoted);
            }
            if voter_weight < _weight {
                return Err(Error::VoterNotHasSoMuchWeight);
            }

            if self.proposals_ids.get(&_proposal) == None {
                return Err(Error::InvalidProposal);
            }
            let proposal_id = self.proposals_ids.get(&_proposal).unwrap();
            let vote_proposal = self
                .vote_proposals
                .get((&election_id, &proposal_id))
                .unwrap();
            self.vote_proposals
                .insert((&election_id, &proposal_id), &(vote_proposal + _weight));
            if voter_weight - _weight == 0 {
                self.voters
                    .insert((&election_id, &voter_address), &(0, true));
            } else {
                self.voters.insert(
                    (&election_id, &voter_address),
                    &(voter_weight - _weight, false),
                );
            }
            Self::env().emit_event(Voted {
                voter: voter_address,
                proposal: _proposal,
                weight: _weight,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn open_registration(&mut self, name: Vec<u8>) -> Result<()> {
            if !self.election_exists(&name) {
                return Err(Error::InesistentElection);
            }
            let election_id = self.elections_ids.get(&name).unwrap();
            if Self::env().caller() != self.elections.get(&election_id).unwrap().0 {
                return Err(Error::OnlyOwner);
            }
            self.elections.get(election_id).unwrap().2 = RegistrationState::RegistationOpen;
            return Ok(());
        }

        #[ink(message)]
        pub fn close_registration(&mut self, name: Vec<u8>) -> Result<()> {
            if !self.election_exists(&name) {
                return Err(Error::InesistentElection);
            }
            let election_id = self.elections_ids.get(&name).unwrap();
            if Self::env().caller() != self.elections.get(&election_id).unwrap().0 {
                return Err(Error::OnlyOwner);
            }
            self.elections.get(election_id).unwrap().2 = RegistrationState::RegistrationClosed;
            return Ok(());
        }

        #[ink(message)]
        pub fn get_number_elections(&self) -> u32 {
            return self.election_count;
        }

        #[ink(message)]
        pub fn get_proposal_for_election(&self, _name: Vec<u8>) -> Vec<Vec<u8>> {
            let election_id = self.elections_ids.get(&_name).unwrap_or_default();
            return self.proposals_list.get(election_id).unwrap_or_default();
        }

        fn election_exists(&self, name: &Vec<u8>) -> bool {
            return self.elections_ids.get(&name) != None;
        }

        #[ink(message)]
        pub fn get_owner_of_election(&self, election_id: u32) -> AccountId {
            return self.elections.get(election_id).unwrap_or_default().0;
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

        /// We test a simple use case of our contract.
        #[ink::test]
        fn create_election_works() {
            let mut ink_voting_dapp = InkVotingDapp::new();
            //let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>();
            assert_eq!(
                ink_voting_dapp.create_election(
                    vec![1, 1, 1],
                    false,
                    vec![vec![12, 3, 3], vec![12, 34]]
                ),
                Ok(())
            );
            assert_eq!(ink_voting_dapp.get_number_elections(), 1);
            assert_eq!(ink_env::test::recorded_events().count(), 1);
            assert_eq!(
                ink_voting_dapp.get_proposal_for_election(vec![1, 1, 1]),
                vec![vec![12, 3, 3], vec![12, 34]]
            );
            assert_eq!(
                ink_voting_dapp.get_owner_of_election(1),
                ink_env::test::default_accounts::<ink_env::DefaultEnvironment>().alice
            );
        }
    }
}
