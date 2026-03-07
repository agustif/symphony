// Verus proof suite for workspace key/path safety contracts.
// Model aligns with rust/crates/symphony-workspace/src/lifecycle.rs.

#![allow(unused_imports)]

use vstd::{prelude::*, seq::*};

verus! {

pub type PathText = Seq<char>;

pub open spec fn is_safe_key_char(c: char) -> bool {
    (c >= 'a' && c <= 'z')
        || (c >= 'A' && c <= 'Z')
        || (c >= '0' && c <= '9')
        || c == '.'
        || c == '_'
        || c == '-'
}

pub open spec fn is_path_separator(c: char) -> bool {
    c == '/' || c == '\\'
}

pub open spec fn workspace_key_sanitized(key: PathText) -> bool {
    forall|i: int| #![auto] 0 <= i < key.len() ==> is_safe_key_char(key[i])
}

pub open spec fn sanitize_workspace_key(identifier: PathText) -> PathText {
    if identifier.len() == 0 {
        seq!['_']
    } else {
        Seq::new(identifier.len(), |i: int| if is_safe_key_char(identifier[i]) {
            identifier[i]
        } else {
            '_'
        })
    }
}

pub open spec fn root_prefix(root: PathText, candidate: PathText) -> bool {
    &&& root.len() <= candidate.len()
    &&& forall|i: int| 0 <= i < root.len() ==> root[i] == candidate[i]
}

pub open spec fn join_workspace_path(root: PathText, key: PathText) -> PathText {
    if root.len() == 0 {
        key
    } else {
        root + seq!['/'] + key
    }
}

pub open spec fn workspace_key_allowed_for_path(key: PathText) -> bool {
    &&& workspace_key_sanitized(key)
    &&& key.len() > 0
    &&& key != seq!['.']
    &&& key != seq!['.', '.']
}

pub proof fn lemma_sanitize_produces_sanitized_key(identifier: PathText)
    ensures
        workspace_key_sanitized(sanitize_workspace_key(identifier)),
{
    if identifier.len() == 0 {
    } else {
        assert forall|i: int|
            #![auto]
            0 <= i < sanitize_workspace_key(identifier).len() implies
                is_safe_key_char(sanitize_workspace_key(identifier)[i]) by {
                if is_safe_key_char(identifier[i]) {
                } else {
                }
            };
    }
}

pub proof fn lemma_sanitize_key_non_empty(identifier: PathText)
    ensures
        sanitize_workspace_key(identifier).len() > 0,
{
    if identifier.len() == 0 {
        assert(sanitize_workspace_key(identifier).len() == 1);
    } else {
        assert(sanitize_workspace_key(identifier).len() == identifier.len());
        assert(identifier.len() > 0);
    }
}

pub proof fn lemma_sanitize_removes_path_separator(identifier: PathText)
    ensures
        forall|i: int|
            0 <= i < sanitize_workspace_key(identifier).len() ==> sanitize_workspace_key(identifier)[i] != '/',
{
    lemma_sanitize_produces_sanitized_key(identifier);
    assert forall|i: int|
        0 <= i < sanitize_workspace_key(identifier).len() implies sanitize_workspace_key(identifier)[i] != '/' by {
            assert(is_safe_key_char(sanitize_workspace_key(identifier)[i]));
        };
}

pub proof fn lemma_sanitize_removes_backslash(identifier: PathText)
    ensures
        forall|i: int|
            0 <= i < sanitize_workspace_key(identifier).len() ==> sanitize_workspace_key(identifier)[i] != '\\',
{
    lemma_sanitize_produces_sanitized_key(identifier);
    assert forall|i: int|
        0 <= i < sanitize_workspace_key(identifier).len() implies sanitize_workspace_key(identifier)[i] != '\\' by {
            assert(is_safe_key_char(sanitize_workspace_key(identifier)[i]));
        };
}

pub proof fn lemma_join_workspace_path_keeps_root_prefix(root: PathText, key: PathText)
    ensures
        root_prefix(root, join_workspace_path(root, key)),
{
    assert(root.len() <= join_workspace_path(root, key).len());
    assert forall|i: int| #![auto] 0 <= i < root.len() implies root[i] == join_workspace_path(root, key)[i] by {
        if root.len() == 0 {
        } else {
        }
    };
}

pub proof fn lemma_dot_keys_rejected_by_workspace_policy()
    ensures
        !workspace_key_allowed_for_path(seq!['.']),
        !workspace_key_allowed_for_path(seq!['.', '.']),
{
}

pub proof fn lemma_safe_non_dot_non_empty_keys_are_allowed(key: PathText)
    requires
        workspace_key_sanitized(key),
        key.len() > 0,
        key != seq!['.'],
        key != seq!['.', '.'],
    ensures
        workspace_key_allowed_for_path(key),
{
}

pub proof fn lemma_empty_and_separator_identifiers_sanitize_to_safe_placeholder()
    ensures
        sanitize_workspace_key(seq![]) == seq!['_'],
        sanitize_workspace_key(seq!['/']) == seq!['_'],
        sanitize_workspace_key(seq!['\\']) == seq!['_'],
        workspace_key_sanitized(sanitize_workspace_key(seq![])),
        workspace_key_sanitized(sanitize_workspace_key(seq!['/'])),
        workspace_key_sanitized(sanitize_workspace_key(seq!['\\'])),
        sanitize_workspace_key(seq![]).len() > 0,
        sanitize_workspace_key(seq!['/']).len() > 0,
        sanitize_workspace_key(seq!['\\']).len() > 0,
{
    assert(sanitize_workspace_key(seq![]) == seq!['_']);
    assert(sanitize_workspace_key(seq!['/']) == seq!['_']);
    assert(sanitize_workspace_key(seq!['\\']) == seq!['_']);
    assert(workspace_key_sanitized(seq!['_'])) by {
        assert forall|i: int| #![auto] 0 <= i < seq!['_'].len() implies is_safe_key_char(seq!['_'][i]) by {
        };
    };
    assert(seq!['_'].len() > 0);
}

pub proof fn lemma_allowed_keys_are_fixed_points_of_sanitize(identifier: PathText)
    requires
        workspace_key_allowed_for_path(identifier),
    ensures
        sanitize_workspace_key(identifier) == identifier,
        workspace_key_allowed_for_path(sanitize_workspace_key(identifier)),
{
    assert(identifier.len() > 0);
    assert forall|i: int| #![auto] 0 <= i < identifier.len() implies sanitize_workspace_key(identifier)[i] == identifier[i] by {
        assert(is_safe_key_char(identifier[i]));
    };
    assert(sanitize_workspace_key(identifier) == identifier);
}

pub proof fn lemma_sanitized_key_is_root_contained_when_allowed(
    root: PathText,
    identifier: PathText,
)
    ensures
        workspace_key_allowed_for_path(sanitize_workspace_key(identifier))
            ==> root_prefix(root, join_workspace_path(root, sanitize_workspace_key(identifier))),
{
    lemma_sanitize_produces_sanitized_key(identifier);
    lemma_sanitize_key_non_empty(identifier);
    lemma_join_workspace_path_keeps_root_prefix(root, sanitize_workspace_key(identifier));
}

pub proof fn lemma_worker_cwd_descendant_is_contained(
    root: PathText,
    key: PathText,
    cwd_suffix: PathText,
)
    requires
        workspace_key_allowed_for_path(key),
    ensures
        root_prefix(root, join_workspace_path(join_workspace_path(root, key), cwd_suffix)),
{
    lemma_join_workspace_path_keeps_root_prefix(root, key);
    lemma_join_workspace_path_keeps_root_prefix(join_workspace_path(root, key), cwd_suffix);
}

fn main() {}

} // verus!
