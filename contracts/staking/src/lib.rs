//! Staking contract.
//!
//! Amounts are represented as `i128` to stay compatible with the token
//! interface used by the underlying SEP-41 token (see issue #872). Because
//! `i128` is signed, every entry point that accepts an amount must reject
//! zero and negative values explicitly with [`Error::InvalidAmount`] rather
//! than relying on a panic or a silent no-op.

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    /// Amount must be strictly positive.
    InvalidAmount = 3,
    /// Arithmetic overflowed the `i128` range.
    Overflow = 4,
    InsufficientBalance = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StakeInfo {
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataKey {
    pub user: Address,
}

#[contract]
pub struct StakingContract;

#[contractimpl]
impl StakingContract {
    /// Stake `amount` for `user`.
    ///
    /// `amount` must be strictly positive; zero and negative values are
    /// rejected with [`Error::InvalidAmount`].
    pub fn stake(env: Env, user: Address, amount: i128) -> Result<(), Error> {
        user.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let key = DataKey { user: user.clone() };
        let current = env
            .storage()
            .persistent()
            .get::<DataKey, StakeInfo>(&key)
            .map(|info| info.amount)
            .unwrap_or(0);

        // Checked arithmetic: the release profile disables overflow checks
        // (see #651), so we must not rely on the compiler here.
        let new_amount = current.checked_add(amount).ok_or(Error::Overflow)?;

        let total = Self::total_staked(env.clone());
        let new_total = total.checked_add(amount).ok_or(Error::Overflow)?;

        env.storage()
            .persistent()
            .set(&key, &StakeInfo { amount: new_amount });
        env.storage().instance().set(&DataKey { user: user.clone() }, &new_total);

        Ok(())
    }

    /// Unstake `amount` for `user`.
    ///
    /// `amount` must be strictly positive; zero and negative values are
    /// rejected with [`Error::InvalidAmount`].
    pub fn unstake(env: Env, user: Address, amount: i128) -> Result<(), Error> {
        user.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let key = DataKey { user: user.clone() };
        let current = env
            .storage()
            .persistent()
            .get::<DataKey, StakeInfo>(&key)
            .map(|info| info.amount)
            .unwrap_or(0);

        if current < amount {
            return Err(Error::InsufficientBalance);
        }

        // Checked arithmetic: the release profile disables overflow checks
        // (see #651), so we must not rely on the compiler here.
        let new_amount = current.checked_sub(amount).ok_or(Error::Overflow)?;

        let total = Self::total_staked(env.clone());
        let new_total = total.checked_sub(amount).ok_or(Error::Overflow)?;

        env.storage()
            .persistent()
            .set(&key, &StakeInfo { amount: new_amount });
        env.storage().instance().set(&DataKey { user: user.clone() }, &new_total);

        Ok(())
    }

    /// Total amount staked across all users.
    pub fn total_staked(env: Env) -> i128 {
        env.storage()
            .instance()
            .get::<DataKey, i128>(&DataKey {
                user: env.current_contract_address(),
            })
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn stake_rejects_zero() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, StakingContract);
        let client = StakingContractClient::new(&env, &contract_id);
        let user = Address::generate(&env);

        assert_eq!(client.stake(&user, &0), Err(Error::InvalidAmount));
    }

    #[test]
    fn stake_rejects_negative() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, StakingContract);
        let client = StakingContractClient::new(&env, &contract_id);
        let user = Address::generate(&env);

        assert_eq!(client.stake(&user, &-1), Err(Error::InvalidAmount));
        assert_eq!(client.stake(&user, &i128::MIN), Err(Error::InvalidAmount));
    }

    #[test]
    fn stake_accepts_i128_max() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, StakingContract);
        let client = StakingContractClient::new(&env, &contract_id);
        let user = Address::generate(&env);

        assert_eq!(client.stake(&user, &i128::MAX), Ok(()));
        assert_eq!(client.total_staked(), i128::MAX);
    }

    #[test]
    fn stake_overflow_is_reported() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, StakingContract);
        let client = StakingContractClient::new(&env, &contract_id);
        let user = Address::generate(&env);

        assert_eq!(client.stake(&user, &i128::MAX), Ok(()));
        assert_eq!(client.stake(&user, &1), Err(Error::Overflow));
    }

    #[test]
    fn unstake_rejects_zero_and_negative() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, StakingContract);
        let client = StakingContractClient::new(&env, &contract_id);
        let user = Address::generate(&env);

        assert_eq!(client.unstake(&user, &0), Err(Error::InvalidAmount));
        assert_eq!(client.unstake(&user, &-1), Err(Error::InvalidAmount));
    }
}
