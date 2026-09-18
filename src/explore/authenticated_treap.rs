//! Small persistent authenticated map for streaming Explore indexes.
//!
//! Tree priority is a domain-separated digest of the key, with the key itself
//! as a total tie-breaker. One key set therefore has one Cartesian-tree shape
//! regardless of insertion order. Updates path-copy `Arc` nodes and rehash only
//! the touched path. This is an operational building block; callers still own
//! the semantic meaning and canonical encoding of keys and value digests.

use std::cmp::Ordering;
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use sha2::{Digest, Sha256};

const AUTHENTICATED_TREAP_VERSION: u32 = 1;
const AUTHENTICATED_TREAP_MAX_DEPTH: u16 = 256;
const PRIORITY_ROLE: &[u8] = b"futuruna.explore.authenticated-treap-priority.v1";
const EMPTY_ROLE: &[u8] = b"futuruna.explore.authenticated-treap-empty.v1";
const NODE_ROLE: &[u8] = b"futuruna.explore.authenticated-treap-node.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AuthenticatedTreapValue {
    digest: [u8; 32],
    weight: u128,
}

impl AuthenticatedTreapValue {
    pub(super) const fn new(digest: [u8; 32], weight: u128) -> Self {
        Self { digest, weight }
    }

    pub(super) const fn digest(self) -> [u8; 32] {
        self.digest
    }

    pub(super) const fn weight(self) -> u128 {
        self.weight
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AuthenticatedTreapMutation {
    Inserted,
    Updated,
    Unchanged,
    Removed,
}

#[derive(Clone, Debug)]
pub(super) struct AuthenticatedTreapMap {
    domain: &'static [u8],
    root: Option<Arc<AuthenticatedTreapNode>>,
}

#[derive(Debug)]
struct AuthenticatedTreapNode {
    key: Box<[u8]>,
    value: AuthenticatedTreapValue,
    priority: [u8; 32],
    left: Option<Arc<Self>>,
    right: Option<Arc<Self>>,
    height: u16,
    entry_count: u128,
    total_weight: u128,
    subtree_hash: [u8; 32],
}

impl AuthenticatedTreapMap {
    pub(super) const fn new(domain: &'static [u8]) -> Self {
        Self { domain, root: None }
    }

    pub(super) fn get(
        &self,
        key: &[u8],
    ) -> Result<Option<AuthenticatedTreapValue>, AuthenticatedTreapError> {
        let mut cursor = self.root.as_deref();
        let mut depth = 0u16;
        while let Some(node) = cursor {
            if depth >= AUTHENTICATED_TREAP_MAX_DEPTH {
                return Err(AuthenticatedTreapError::DepthLimit);
            }
            depth += 1;
            match key.cmp(node.key.as_ref()) {
                Ordering::Less => cursor = node.left.as_deref(),
                Ordering::Greater => cursor = node.right.as_deref(),
                Ordering::Equal => return Ok(Some(node.value)),
            }
        }
        Ok(None)
    }

    pub(super) fn insert(
        &mut self,
        key: impl Into<Box<[u8]>>,
        value: AuthenticatedTreapValue,
    ) -> Result<AuthenticatedTreapMutation, AuthenticatedTreapError> {
        let key = key.into();
        if self.get(&key)?.is_some() {
            return Err(AuthenticatedTreapError::ExistingKey);
        }
        let entry = TreapEntry::new(self.domain, key, value);
        self.root = Some(insert_absent(self.domain, self.root.clone(), entry, 0)?);
        Ok(AuthenticatedTreapMutation::Inserted)
    }

    pub(super) fn update(
        &mut self,
        key: &[u8],
        value: AuthenticatedTreapValue,
    ) -> Result<AuthenticatedTreapMutation, AuthenticatedTreapError> {
        let Some(existing) = self.get(key)? else {
            return Err(AuthenticatedTreapError::MissingKey);
        };
        if existing == value {
            return Ok(AuthenticatedTreapMutation::Unchanged);
        }
        self.root = Some(update_existing(
            self.domain,
            self.root.clone().expect("existing key requires a root"),
            key,
            value,
            0,
        )?);
        Ok(AuthenticatedTreapMutation::Updated)
    }

    pub(super) fn remove(
        &mut self,
        key: &[u8],
    ) -> Result<AuthenticatedTreapMutation, AuthenticatedTreapError> {
        if self.get(key)?.is_none() {
            return Err(AuthenticatedTreapError::MissingKey);
        }
        self.root = remove_existing(self.domain, self.root.clone(), key, 0)?;
        Ok(AuthenticatedTreapMutation::Removed)
    }

    pub(super) fn root_hash(&self) -> [u8; 32] {
        subtree_hash(self.domain, &self.root)
    }

    pub(super) fn entry_count(&self) -> u128 {
        subtree_entry_count(&self.root)
    }

    pub(super) fn total_weight(&self) -> u128 {
        subtree_total_weight(&self.root)
    }

    /// Borrow the entry at one canonical key-order ordinal without flattening
    /// or cloning the authenticated map. This is the bounded publication path
    /// for large, already closed semantic indexes.
    pub(super) fn entry_at_ordinal(
        &self,
        mut ordinal: u128,
    ) -> Result<Option<(&[u8], AuthenticatedTreapValue)>, AuthenticatedTreapError> {
        let mut cursor = self.root.as_deref();
        let mut depth = 0u16;
        while let Some(node) = cursor {
            if depth >= AUTHENTICATED_TREAP_MAX_DEPTH {
                return Err(AuthenticatedTreapError::DepthLimit);
            }
            depth += 1;
            let left_count = subtree_entry_count(&node.left);
            if ordinal < left_count {
                cursor = node.left.as_deref();
            } else if ordinal == left_count {
                return Ok(Some((node.key.as_ref(), node.value)));
            } else {
                ordinal = ordinal
                    .checked_sub(left_count)
                    .and_then(|value| value.checked_sub(1))
                    .ok_or(AuthenticatedTreapError::AggregateOverflow)?;
                cursor = node.right.as_deref();
            }
        }
        Ok(None)
    }
}

#[derive(Clone)]
struct TreapEntry {
    key: Box<[u8]>,
    value: AuthenticatedTreapValue,
    priority: [u8; 32],
}

impl TreapEntry {
    fn new(domain: &'static [u8], key: Box<[u8]>, value: AuthenticatedTreapValue) -> Self {
        let priority = derive_priority(domain, &key);
        Self {
            key,
            value,
            priority,
        }
    }
}

impl AuthenticatedTreapNode {
    fn from_entry(
        domain: &'static [u8],
        entry: TreapEntry,
        left: Option<Arc<Self>>,
        right: Option<Arc<Self>>,
    ) -> Result<Arc<Self>, AuthenticatedTreapError> {
        Self::new(domain, entry.key, entry.value, entry.priority, left, right)
    }

    fn rebuild(
        domain: &'static [u8],
        source: &Self,
        left: Option<Arc<Self>>,
        right: Option<Arc<Self>>,
    ) -> Result<Arc<Self>, AuthenticatedTreapError> {
        Self::new(
            domain,
            source.key.clone(),
            source.value,
            source.priority,
            left,
            right,
        )
    }

    fn new(
        domain: &'static [u8],
        key: Box<[u8]>,
        value: AuthenticatedTreapValue,
        priority: [u8; 32],
        left: Option<Arc<Self>>,
        right: Option<Arc<Self>>,
    ) -> Result<Arc<Self>, AuthenticatedTreapError> {
        let height = subtree_height(&left)
            .max(subtree_height(&right))
            .checked_add(1)
            .ok_or(AuthenticatedTreapError::DepthLimit)?;
        if height > AUTHENTICATED_TREAP_MAX_DEPTH {
            return Err(AuthenticatedTreapError::DepthLimit);
        }
        let entry_count = subtree_entry_count(&left)
            .checked_add(1)
            .and_then(|count| count.checked_add(subtree_entry_count(&right)))
            .ok_or(AuthenticatedTreapError::AggregateOverflow)?;
        let total_weight = subtree_total_weight(&left)
            .checked_add(value.weight)
            .and_then(|weight| weight.checked_add(subtree_total_weight(&right)))
            .ok_or(AuthenticatedTreapError::AggregateOverflow)?;
        let left_hash = subtree_hash(domain, &left);
        let right_hash = subtree_hash(domain, &right);
        let mut hasher = TreapHasher::new(NODE_ROLE, domain);
        hasher.u32(AUTHENTICATED_TREAP_VERSION);
        hasher.digest(left_hash);
        hasher.u128(subtree_entry_count(&left));
        hasher.u128(subtree_total_weight(&left));
        hasher.bytes(&key);
        hasher.digest(value.digest);
        hasher.u128(value.weight);
        hasher.digest(priority);
        hasher.digest(right_hash);
        hasher.u128(subtree_entry_count(&right));
        hasher.u128(subtree_total_weight(&right));
        hasher.u128(entry_count);
        hasher.u128(total_weight);
        Ok(Arc::new(Self {
            key,
            value,
            priority,
            left,
            right,
            height,
            entry_count,
            total_weight,
            subtree_hash: hasher.finish(),
        }))
    }
}

fn insert_absent(
    domain: &'static [u8],
    root: Option<Arc<AuthenticatedTreapNode>>,
    entry: TreapEntry,
    depth: u16,
) -> Result<Arc<AuthenticatedTreapNode>, AuthenticatedTreapError> {
    let Some(root) = root else {
        return AuthenticatedTreapNode::from_entry(domain, entry, None, None);
    };
    check_depth(depth)?;
    if entry_precedes(&entry, &root) {
        let (left, right) = split(domain, Some(root), &entry.key, depth + 1)?;
        return AuthenticatedTreapNode::from_entry(domain, entry, left, right);
    }
    match entry.key.as_ref().cmp(root.key.as_ref()) {
        Ordering::Less => AuthenticatedTreapNode::rebuild(
            domain,
            &root,
            Some(insert_absent(domain, root.left.clone(), entry, depth + 1)?),
            root.right.clone(),
        ),
        Ordering::Greater => AuthenticatedTreapNode::rebuild(
            domain,
            &root,
            root.left.clone(),
            Some(insert_absent(domain, root.right.clone(), entry, depth + 1)?),
        ),
        Ordering::Equal => Err(AuthenticatedTreapError::ExistingKey),
    }
}

fn split(
    domain: &'static [u8],
    root: Option<Arc<AuthenticatedTreapNode>>,
    key: &[u8],
    depth: u16,
) -> Result<
    (
        Option<Arc<AuthenticatedTreapNode>>,
        Option<Arc<AuthenticatedTreapNode>>,
    ),
    AuthenticatedTreapError,
> {
    let Some(root) = root else {
        return Ok((None, None));
    };
    check_depth(depth)?;
    if root.key.as_ref() < key {
        let (middle, right) = split(domain, root.right.clone(), key, depth + 1)?;
        Ok((
            Some(AuthenticatedTreapNode::rebuild(
                domain,
                &root,
                root.left.clone(),
                middle,
            )?),
            right,
        ))
    } else {
        let (left, middle) = split(domain, root.left.clone(), key, depth + 1)?;
        Ok((
            left,
            Some(AuthenticatedTreapNode::rebuild(
                domain,
                &root,
                middle,
                root.right.clone(),
            )?),
        ))
    }
}

fn update_existing(
    domain: &'static [u8],
    root: Arc<AuthenticatedTreapNode>,
    key: &[u8],
    value: AuthenticatedTreapValue,
    depth: u16,
) -> Result<Arc<AuthenticatedTreapNode>, AuthenticatedTreapError> {
    check_depth(depth)?;
    match key.cmp(root.key.as_ref()) {
        Ordering::Less => AuthenticatedTreapNode::rebuild(
            domain,
            &root,
            Some(update_existing(
                domain,
                root.left
                    .clone()
                    .ok_or(AuthenticatedTreapError::MissingKey)?,
                key,
                value,
                depth + 1,
            )?),
            root.right.clone(),
        ),
        Ordering::Greater => AuthenticatedTreapNode::rebuild(
            domain,
            &root,
            root.left.clone(),
            Some(update_existing(
                domain,
                root.right
                    .clone()
                    .ok_or(AuthenticatedTreapError::MissingKey)?,
                key,
                value,
                depth + 1,
            )?),
        ),
        Ordering::Equal => AuthenticatedTreapNode::new(
            domain,
            root.key.clone(),
            value,
            root.priority,
            root.left.clone(),
            root.right.clone(),
        ),
    }
}

fn remove_existing(
    domain: &'static [u8],
    root: Option<Arc<AuthenticatedTreapNode>>,
    key: &[u8],
    depth: u16,
) -> Result<Option<Arc<AuthenticatedTreapNode>>, AuthenticatedTreapError> {
    let root = root.ok_or(AuthenticatedTreapError::MissingKey)?;
    check_depth(depth)?;
    match key.cmp(root.key.as_ref()) {
        Ordering::Less => Ok(Some(AuthenticatedTreapNode::rebuild(
            domain,
            &root,
            remove_existing(domain, root.left.clone(), key, depth + 1)?,
            root.right.clone(),
        )?)),
        Ordering::Greater => Ok(Some(AuthenticatedTreapNode::rebuild(
            domain,
            &root,
            root.left.clone(),
            remove_existing(domain, root.right.clone(), key, depth + 1)?,
        )?)),
        Ordering::Equal => merge(domain, root.left.clone(), root.right.clone(), depth + 1),
    }
}

fn merge(
    domain: &'static [u8],
    left: Option<Arc<AuthenticatedTreapNode>>,
    right: Option<Arc<AuthenticatedTreapNode>>,
    depth: u16,
) -> Result<Option<Arc<AuthenticatedTreapNode>>, AuthenticatedTreapError> {
    match (left, right) {
        (None, right) => Ok(right),
        (left, None) => Ok(left),
        (Some(left), Some(right)) if node_precedes(&left, &right) => {
            check_depth(depth)?;
            Ok(Some(AuthenticatedTreapNode::rebuild(
                domain,
                &left,
                left.left.clone(),
                merge(domain, left.right.clone(), Some(right), depth + 1)?,
            )?))
        }
        (Some(left), Some(right)) => {
            check_depth(depth)?;
            Ok(Some(AuthenticatedTreapNode::rebuild(
                domain,
                &right,
                merge(domain, Some(left), right.left.clone(), depth + 1)?,
                right.right.clone(),
            )?))
        }
    }
}

fn entry_precedes(entry: &TreapEntry, node: &AuthenticatedTreapNode) -> bool {
    (entry.priority, entry.key.as_ref()) < (node.priority, node.key.as_ref())
}

fn node_precedes(left: &AuthenticatedTreapNode, right: &AuthenticatedTreapNode) -> bool {
    (left.priority, left.key.as_ref()) < (right.priority, right.key.as_ref())
}

fn derive_priority(domain: &'static [u8], key: &[u8]) -> [u8; 32] {
    let mut hasher = TreapHasher::new(PRIORITY_ROLE, domain);
    hasher.u32(AUTHENTICATED_TREAP_VERSION);
    hasher.bytes(key);
    hasher.finish()
}

fn subtree_hash(domain: &'static [u8], root: &Option<Arc<AuthenticatedTreapNode>>) -> [u8; 32] {
    root.as_ref().map_or_else(
        || {
            let mut hasher = TreapHasher::new(EMPTY_ROLE, domain);
            hasher.u32(AUTHENTICATED_TREAP_VERSION);
            hasher.finish()
        },
        |node| node.subtree_hash,
    )
}

fn subtree_height(root: &Option<Arc<AuthenticatedTreapNode>>) -> u16 {
    root.as_ref().map_or(0, |node| node.height)
}

fn subtree_entry_count(root: &Option<Arc<AuthenticatedTreapNode>>) -> u128 {
    root.as_ref().map_or(0, |node| node.entry_count)
}

fn subtree_total_weight(root: &Option<Arc<AuthenticatedTreapNode>>) -> u128 {
    root.as_ref().map_or(0, |node| node.total_weight)
}

fn check_depth(depth: u16) -> Result<(), AuthenticatedTreapError> {
    if depth >= AUTHENTICATED_TREAP_MAX_DEPTH {
        Err(AuthenticatedTreapError::DepthLimit)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthenticatedTreapError {
    ExistingKey,
    MissingKey,
    AggregateOverflow,
    DepthLimit,
}

impl fmt::Display for AuthenticatedTreapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ExistingKey => "authenticated treap key already exists",
            Self::MissingKey => "authenticated treap key does not exist",
            Self::AggregateOverflow => "authenticated treap aggregate overflowed",
            Self::DepthLimit => "authenticated treap exceeded its fail-closed depth limit",
        })
    }
}

impl Error for AuthenticatedTreapError {}

struct TreapHasher {
    hasher: Sha256,
}

impl TreapHasher {
    fn new(role: &[u8], domain: &[u8]) -> Self {
        let mut value = Self {
            hasher: Sha256::new(),
        };
        value.bytes(role);
        value.bytes(domain);
        value
    }

    fn u32(&mut self, value: u32) {
        self.hasher.update(value.to_be_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.hasher.update(value.to_be_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.u128(bytes.len() as u128);
        self.hasher.update(bytes);
    }

    fn digest(&mut self, digest: [u8; 32]) {
        self.hasher.update(digest);
    }

    fn finish(self) -> [u8; 32] {
        self.hasher.finalize().into()
    }
}
