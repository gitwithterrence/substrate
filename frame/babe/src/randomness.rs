// This file is part of Substrate.

// Copyright (C) 2019-2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Provides multiple implementations of the randomness trait based on the on-chain session
//! randomness collected from VRF outputs.

use super::{
	AuthorVrfRandomness, Config, SessionStart, NextRandomness, Randomness, VRF_OUTPUT_LENGTH,
};
use frame_support::traits::Randomness as RandomnessT;
use sp_runtime::traits::Hash;

/// Randomness usable by consensus protocols that **depend** upon finality and take action
/// based upon on-chain commitments made during the session before the previous session.
///
/// An off-chain consensus protocol requires randomness be finalized before usage, but one
/// extra session delay beyond `RandomnessFromOneSessionAgo` suffices, under the assumption
/// that finality never stalls for longer than one session.
///
/// All randomness is relative to commitments to any other inputs to the computation: If
/// Alice samples randomness near perfectly using radioactive decay, but then afterwards
/// Eve selects an arbitrary value with which to xor Alice's randomness, then Eve always
/// wins whatever game they play.
///
/// All input commitments used with `RandomnessFromTwoSessionsAgo` should come from at least
/// three sessions ago. We require BABE session keys be registered at least three sessions
/// before being used to derive `CurrentBlockRandomness` for example.
///
/// All users learn `RandomnessFromTwoSessionsAgo` when session `current_session - 1` starts,
/// although some learn it a few block earlier inside session `current_session - 2`.
///
/// Adversaries with enough block producers could bias this randomness by choosing upon
/// what their block producers build at the end of session `current_session - 2` or the
/// beginning session `current_session - 1`, or skipping slots at the end of session
/// `current_session - 2`.
///
/// Adversaries should not possess many block production slots towards the beginning or
/// end of every session, but they possess some influence over when they possess more slots.
pub struct RandomnessFromTwoSessionsAgo<T>(sp_std::marker::PhantomData<T>);

/// Randomness usable by on-chain code that **does not depend** upon finality and takes
/// action based upon on-chain commitments made during the previous session.
///
/// All randomness is relative to commitments to any other inputs to the computation: If
/// Alice samples randomness near perfectly using radioactive decay, but then afterwards
/// Eve selects an arbitrary value with which to xor Alice's randomness, then Eve always
/// wins whatever game they play.
///
/// All input commitments used with `RandomnessFromOneSessionAgo` should come from at least
/// two sessions ago, although the previous session might work in special cases under
/// additional assumption.
///
/// All users learn `RandomnessFromOneSessionAgo` at the end of the previous session, although
/// some block producers learn it several block earlier.
///
/// Adversaries with enough block producers could bias this randomness by choosing upon
/// what their block producers build at either the end of the previous session or the
/// beginning of the current session, or electing to skipping some of their own block
/// production slots towards the end of the previous session.
///
/// Adversaries should not possess many block production slots towards the beginning or
/// end of every session, but they possess some influence over when they possess more slots.
///
/// As an example usage, we determine parachain auctions ending times in Polkadot using
/// `RandomnessFromOneSessionAgo` because it reduces bias from `CurrentBlockRandomness` and
/// does not require the extra finality delay of `RandomnessFromTwoSessionsAgo`.
pub struct RandomnessFromOneSessionAgo<T>(sp_std::marker::PhantomData<T>);

/// Randomness produced semi-freshly with each block, but inherits limitations of
/// `RandomnessFromTwoSessionsAgo` from which it derives.
///
/// All randomness is relative to commitments to any other inputs to the computation: If
/// Alice samples randomness near perfectly using radioactive decay, but then afterwards
/// Eve selects an arbitrary value with which to xor Alice's randomness, then Eve always
/// wins whatever game they play.
///
/// As with `RandomnessFromTwoSessionsAgo`, all input commitments combined with
/// `CurrentBlockRandomness` should come from at least two session ago, except preferably
/// not near session ending, and thus ideally three sessions ago.
///
/// Almost all users learn this randomness for a block when the block producer announces
/// the block, which makes this randomness appear quite fresh. Yet, the block producer
/// themselves learned this randomness at the beginning of session `current_session - 2`, at
/// the same time as they learn `RandomnessFromTwoSessionsAgo`.
///
/// Aside from just biasing `RandomnessFromTwoSessionsAgo`, adversaries could also bias
/// `CurrentBlockRandomness` by never announcing their block if doing so yields an
/// unfavorable randomness. As such, `CurrentBlockRandomness` should be considered weaker
/// than both other randomness sources provided by BABE, but `CurrentBlockRandomness`
/// remains constrained by declared staking, while a randomness source like block hash is
/// only constrained by adversaries' unknowable computational power.
///
/// As an example use, parachains could assign block production slots based upon the
/// `CurrentBlockRandomness` of their relay parent or relay parent's parent, provided the
/// parachain registers collators but avoids censorship sensitive functionality like
/// slashing. Any parachain with slashing could operate BABE itself or perhaps better yet
/// a BABE-like approach that derives its `CurrentBlockRandomness`, and authorizes block
/// production, based upon the relay parent's `CurrentBlockRandomness` or more likely the
/// relay parent's `RandomnessFromTwoSessionsAgo`.
pub struct CurrentBlockRandomness<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> RandomnessT<T::Hash, T::BlockNumber> for RandomnessFromTwoSessionsAgo<T> {
	fn random(subject: &[u8]) -> (T::Hash, T::BlockNumber) {
		let mut subject = subject.to_vec();
		subject.reserve(VRF_OUTPUT_LENGTH);
		subject.extend_from_slice(&Randomness::<T>::get()[..]);

		(T::Hashing::hash(&subject[..]), SessionStart::<T>::get().0)
	}
}

impl<T: Config> RandomnessT<T::Hash, T::BlockNumber> for RandomnessFromOneSessionAgo<T> {
	fn random(subject: &[u8]) -> (T::Hash, T::BlockNumber) {
		let mut subject = subject.to_vec();
		subject.reserve(VRF_OUTPUT_LENGTH);
		subject.extend_from_slice(&NextRandomness::<T>::get()[..]);

		(T::Hashing::hash(&subject[..]), SessionStart::<T>::get().1)
	}
}

impl<T: Config> RandomnessT<Option<T::Hash>, T::BlockNumber> for CurrentBlockRandomness<T> {
	fn random(subject: &[u8]) -> (Option<T::Hash>, T::BlockNumber) {
		let random = AuthorVrfRandomness::<T>::get().map(|random| {
			let mut subject = subject.to_vec();
			subject.reserve(VRF_OUTPUT_LENGTH);
			subject.extend_from_slice(&random);

			T::Hashing::hash(&subject[..])
		});

		(random, <frame_system::Pallet<T>>::block_number())
	}
}
