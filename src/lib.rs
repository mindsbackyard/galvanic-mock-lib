/* Copyright 2017 Christopher Bacher
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! The support library for **[galvanic-mock](https://www.github.com/mindsbackyard/galvanic-mock)**.
//!
//! The crate provides common traits for all mocks generated by **galvanic-mock** as well as data structures for handling the state of mock objects.

#[cfg(feature = "galvanic_assert_integration")] extern crate galvanic_assert;

use std::collections::HashMap;
use std::cell::RefCell;

/// A trait for controlling the behaviour of a mock.
///
/// All mocks generated by `galvanic-mock` implement this trait.
/// The generated mocks use a `MockState` object internally to handle the state of the mock.
/// The mock's implementation of the `MockControl` trait acts as a proxy to the `MockState` object.
pub trait MockControl {
    /// Passing `true` enables verification of expected behaviours when the mock object is dropped.
    ///
    /// See `verify()`.
    fn should_verify_on_drop(&mut self, flag: bool);

    /// For *internal* use only.
    ///
    /// Enables a behaviour defined in a `given!`-block for a trait's method.
    ///
    /// # Arguments
    /// * `requested_trait` - the trait's name
    /// * `method` - the trait's method's name
    /// * `behaviour` - the behaviour to be activated
    fn add_given_behaviour(&self,
                           requested_trait: &'static str,
                           method: &'static str,
                           behaviour: GivenBehaviour);

    /// Deactivates all behaviours activated by a `given!`-block before.
    fn reset_given_behaviours(&mut self);

    /// For *internal* use only.
    ///
    /// Enables a behaviour defined in a `expect_interactions!`-block for a trait's method.
    ///
    /// # Arguments
    /// * `requested_trait` - the trait's name
    /// * `method` - the trait's method's name
    /// * `behaviour` - the behaviour to be activated
    fn add_expect_behaviour(&self,
                            requested_trait: &'static str,
                            method: &'static str,
                            behaviour: ExpectBehaviour);

    /// Deactivates all behaviours activated by a `expect_interactions!`-block before.
    fn reset_expected_behaviours(&mut self);

    /// Returns `true` iff all expected interactions with the mock have occurred.
    fn are_expected_behaviours_satisfied(&self) -> bool;

    /// Panics if some expected interaction with the mock has not occurred.
    ///
    /// An expected interaction is defined by a behaviour added to the mock in an `expect_interactions!`-block.
    /// A behaviour is said to match if the method asocciated with the behaviour is called with values satisfying the behaviour's argument pattern.
    /// A behaviour is satisfied if its expected repetitions are fulfilled.
    ///
    /// Verification should be skipped if the current thread is already panicking.
    /// This method should also be executed once the implementor is drpped if verication on drop is enabled.
    fn verify(&self);
}


/// Stores the state of a mock.
///
/// The state of a mock object is compromised by its enabled *given* and *expected* behaviours.
/// As well as its verification policies.
pub struct MockState {
    /// The enabled *given* behaviours addressed by a tuple of the names of the mocked *trait* and *method*.
    pub given_behaviours: RefCell<HashMap<(&'static str, &'static str), Vec<GivenBehaviour>>>,
    /// The enabled *expected* behaviours addressed by a tuple of the names of the mocked *trait* and *method*.
    pub expect_behaviours: RefCell<HashMap<(&'static str, &'static str), Vec<ExpectBehaviour>>>,
    /// Whether the *expected behaviours should be verfied on drop.
    verify_on_drop: bool,
}

impl MockState {
    pub fn new() -> Self {
        Self {
            given_behaviours: RefCell::new(HashMap::new()),
            expect_behaviours: RefCell::new(HashMap::new()),
            verify_on_drop: true,
        }
    }
}

impl MockControl for MockState {
    fn should_verify_on_drop(&mut self, flag: bool) {
        self.verify_on_drop = flag;
    }

    fn add_given_behaviour(&self,
                           requested_trait: &'static str,
                           method: &'static str,
                           behaviour: GivenBehaviour) {
        self.given_behaviours
            .borrow_mut()
            .entry((requested_trait, method))
            .or_insert_with(|| Vec::new())
            .push(behaviour);
    }

    fn reset_given_behaviours(&mut self) {
        self.given_behaviours.borrow_mut().clear();
    }

    fn add_expect_behaviour(&self,
                            requested_trait: &'static str,
                            method: &'static str,
                            behaviour: ExpectBehaviour) {
        self.expect_behaviours
            .borrow_mut()
            .entry((requested_trait, method))
            .or_insert_with(|| Vec::new())
            .push(behaviour);
    }

    fn reset_expected_behaviours(&mut self) {
        self.expect_behaviours.borrow_mut().clear();
    }

    fn are_expected_behaviours_satisfied(&self) -> bool {
        let mut unsatisfied_messages: Vec<String> = Vec::new();
        for behaviour in self.expect_behaviours.borrow().values().flat_map(|vs| vs) {
            if !behaviour.is_saturated() {
                unsatisfied_messages
                    .push(format!("Behaviour unsatisfied with {} matching invocations: {}",
                                  behaviour.num_matches.get(),
                                  behaviour.describe()));
            }
        }
        if !unsatisfied_messages.is_empty() {
            for message in unsatisfied_messages {
                eprintln!("{}", message);
            }
            false
        } else {
            true
        }
    }

    fn verify(&self) {
        if !std::thread::panicking() && !self.are_expected_behaviours_satisfied() {
            panic!("There are unsatisfied expected behaviours for mocked traits.");
        }
    }
}

impl std::ops::Drop for MockState {
    /// Verfies the *expected interactions* on the mock if the policy is enabled.
    ///
    /// # Panics
    /// iff the verification fails.
    fn drop(&mut self) {
        if self.verify_on_drop {
            self.verify();
        }
    }
}


/// Defines a matcher for the arguments of a mocked method.
///
/// A matcher may check a single argument of a method or all arguments at once.
/// If all arguments are to be checked then they should be passed in a curried form, e.g., as a tuple.
pub trait ArgMatcher<'a, T: 'a> {
    // Returns `true` iff the `actual` arguments satisfy the matcher.
    fn match_args(&self, actual: &'a T) -> bool;
}

/// Any function accepting an argument and returning a `bool` can be used as `ArgMatcher`.
impl<'a, T: 'a, F> ArgMatcher<'a, T> for F
    where F: Fn(&'a T) -> bool
{
    fn match_args(&self, actual: &'a T) -> bool {
        self(actual)
    }
}

/// All matchers of the **galvanic-assert** crate can be used as `ArgMatcher`.
///
/// The crate's matchers can either be used to inspect a single argument or all of them (in curried form).
#[cfg(feature = "galvanic_assert_integration")]
impl<'a, T: 'a> ArgMatcher<'a, T> for Box<::galvanic_assert::Matcher<'a, T> + 'a> {
    fn match_args(&self, actual: &'a T) -> bool {
        self.check(actual).into()
    }
}

/// Stores the state of a *given* behaviour.
pub struct GivenBehaviour {
    /// The unique id of the behaviour within the mocked method to which it belongs.
    pub stmt_id: usize,
    /// How often the behaviour has been matched.
    num_matches: std::cell::Cell<usize>,
    /// How often the behaviour should be matched before it is exhausted, `None` if never.
    expected_matches: Option<usize>,
    /// The bound variables available to the behaviour's `ArgMatcher`.
    pub bound: std::rc::Rc<std::any::Any>,
    /// A string representation of the behaviour's definition.
    stmt_repr: String,
}

impl GivenBehaviour {
    /// Creates a new behaviour which is never exhausted.
    pub fn with(stmt_id: usize, bound: std::rc::Rc<std::any::Any>, stmt_repr: &str) -> Self {
        Self {
            stmt_id: stmt_id,
            num_matches: std::cell::Cell::new(0),
            expected_matches: None,
            bound: bound,
            stmt_repr: stmt_repr.to_string(),
        }
    }

    /// Creates a new behaviour which is never exhausted after being matched `times`.
    pub fn with_times(times: usize,
                      stmt_id: usize,
                      bound: std::rc::Rc<std::any::Any>,
                      stmt_repr: &str)
                      -> Self {
        Self {
            stmt_id: stmt_id,
            num_matches: std::cell::Cell::new(0),
            expected_matches: Some(times),
            bound: bound,
            stmt_repr: stmt_repr.to_string(),
        }
    }

    /// Notifies the behaviour that it has been matched.
    pub fn matched(&self) {
        self.num_matches.set(self.num_matches.get() + 1);
    }

    /// Returns `true` iff the behaviour is exhausted.
    pub fn is_saturated(&self) -> bool {
        match self.expected_matches {
            Some(limit) => self.num_matches.get() >= limit,
            None => false,
        }
    }

    /// Returns a description of the behaviour.
    pub fn describe(&self) -> &str {
        &self.stmt_repr
    }
}


/// Stores the state of a *expected* behaviour.
pub struct ExpectBehaviour {
    /// The unique id of the behaviour within the mocked method to which it belongs.
    pub stmt_id: usize,
    /// How often the behaviour has been matched.
    num_matches: std::cell::Cell<usize>,
    /// The expected minimum number of matches for the behaviour to be satisfied
    expected_min_matches: Option<usize>,
    /// The expected maximum number of matches for the behaviour to be satisfied
    expected_max_matches: Option<usize>,
    #[allow(dead_code)] in_order: Option<bool>,
    /// The bound variables available to the behaviour's `ArgMatcher`.
    pub bound: std::rc::Rc<std::any::Any>,
    /// A string representation of the behaviour's definition.
    stmt_repr: String,
}


impl ExpectBehaviour {
    /// Creates a new behaviour which is satisfied if matched `times`.
    pub fn with_times(times: usize,
                      stmt_id: usize,
                      bound: std::rc::Rc<std::any::Any>,
                      stmt_repr: &str)
                      -> Self {
        Self {
            stmt_id: stmt_id,
            num_matches: std::cell::Cell::new(0),
            expected_min_matches: Some(times),
            expected_max_matches: Some(times),
            in_order: None,
            bound: bound,
            stmt_repr: stmt_repr.to_string(),
        }
    }

    /// Creates a new behaviour which is satisfied if matched `at_least_times`.
    pub fn with_at_least(at_least_times: usize,
                         stmt_id: usize,
                         bound: std::rc::Rc<std::any::Any>,
                         stmt_repr: &str)
                         -> Self {
        Self {
            stmt_id: stmt_id,
            num_matches: std::cell::Cell::new(0),
            expected_min_matches: Some(at_least_times),
            expected_max_matches: None,
            in_order: None,
            bound: bound,
            stmt_repr: stmt_repr.to_string(),
        }
    }

    /// Creates a new behaviour which is satisfied if matched `at_most_times`.
    pub fn with_at_most(at_most_times: usize,
                        stmt_id: usize,
                        bound: std::rc::Rc<std::any::Any>,
                        stmt_repr: &str)
                        -> Self {
        Self {
            stmt_id: stmt_id,
            num_matches: std::cell::Cell::new(0),
            expected_min_matches: None,
            expected_max_matches: Some(at_most_times),
            in_order: None,
            bound: bound,
            stmt_repr: stmt_repr.to_string(),
        }
    }

    /// Creates a new behaviour which is satisfied if matched between `[at_least_times, at_most_times]` (inclusive endpoints).
    pub fn with_between(at_least_times: usize,
                        at_most_times: usize,
                        stmt_id: usize,
                        bound: std::rc::Rc<std::any::Any>,
                        stmt_repr: &str)
                        -> Self {
        Self {
            stmt_id: stmt_id,
            num_matches: std::cell::Cell::new(0),
            expected_min_matches: Some(at_least_times),
            expected_max_matches: Some(at_most_times),
            in_order: None,
            bound: bound,
            stmt_repr: stmt_repr.to_string(),
        }
    }

    /// Notifies the behaviour that it has been matched.
    pub fn matched(&self) {
        self.num_matches.set(self.num_matches.get() + 1);
    }

    /// Returns `true` iff current number of matches would satify the behaviours expected repetitions.
    pub fn is_saturated(&self) -> bool {
        self.expected_min_matches.unwrap_or(0) <= self.num_matches.get() &&
        self.num_matches.get() <= self.expected_max_matches.unwrap_or(std::usize::MAX)
    }

    /// Returns a description of the behaviour.
    pub fn describe(&self) -> &str {
        &self.stmt_repr
    }
}
