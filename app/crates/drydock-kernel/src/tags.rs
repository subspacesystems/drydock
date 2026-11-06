// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Tags: named labels with a visible flag. A group or component instance carries at most
//! one tag; an instance whose tag is hidden is not shown (drawing, picking and export leave
//! it out; that part lives outside the kernel).
//!
//! Behaviour: `docs/specs/m10-tags-kernel.md`. Every change is a `TagRecord` inside
//! `ModelRecord::Tag`, applied by `Model::apply` and undone by its inverse.

use std::collections::BTreeMap;

use crate::model::{BodyId, Instance, InstanceId, Model, ModelEdit, ModelError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TagId(pub u64);

/// A tag's name and whether instances carrying it are shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    /// Any text that isn't empty or only whitespace (`tag_name_is_good`).
    pub name: String,
    pub visible: bool,
}

impl Tag {
    /// A visible tag.
    pub fn new(name: &str) -> Tag {
        Tag {
            name: name.to_string(),
            visible: true,
        }
    }
}

/// Whether `name` can name a tag: not empty and not only whitespace.
pub fn tag_name_is_good(name: &str) -> bool {
    !name.trim().is_empty()
}

/// One primitive change to the model's tags. `Remove*` and `Set*` records carry the old
/// data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagRecord {
    AddTag(TagId, Tag),
    RemoveTag(TagId, Tag),
    /// Rename or show/hide: the whole tag before and after.
    SetTag {
        id: TagId,
        from: Tag,
        to: Tag,
    },
    /// The tag an instance carries, `None` for untagged.
    SetInstanceTag {
        instance: InstanceId,
        from: Option<TagId>,
        to: Option<TagId>,
    },
}

impl TagRecord {
    /// The record that undoes this one.
    pub fn inverse(&self) -> TagRecord {
        match self {
            TagRecord::AddTag(id, tag) => TagRecord::RemoveTag(*id, tag.clone()),
            TagRecord::RemoveTag(id, tag) => TagRecord::AddTag(*id, tag.clone()),
            TagRecord::SetTag { id, from, to } => TagRecord::SetTag {
                id: *id,
                from: to.clone(),
                to: from.clone(),
            },
            TagRecord::SetInstanceTag { instance, from, to } => TagRecord::SetInstanceTag {
                instance: *instance,
                from: *to,
                to: *from,
            },
        }
    }
}

/// The model's tags and which instance carries which. Equality compares the tags and the
/// assignments, not the id counter.
#[derive(Clone, Debug, Default)]
pub struct Tags {
    tags: BTreeMap<TagId, Tag>,
    assigned: BTreeMap<InstanceId, TagId>,
    /// The id `Model::add_tag` hands out next; always past every id ever added.
    #[allow(dead_code)] // skeleton: used by the tag operations (m10-tags-kernel)
    next: u64,
}

impl PartialEq for Tags {
    fn eq(&self, other: &Self) -> bool {
        self.tags == other.tags && self.assigned == other.assigned
    }
}

impl Tags {
    pub fn get(&self, id: TagId) -> Option<&Tag> {
        self.tags.get(&id)
    }

    /// Tags in ascending id order.
    pub fn iter(&self) -> impl Iterator<Item = (TagId, &Tag)> {
        self.tags.iter().map(|(id, tag)| (*id, tag))
    }

    pub fn len(&self) -> usize {
        self.tags.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    /// The tag the instance carries; `None` when untagged or unknown.
    pub fn instance_tag(&self, instance: InstanceId) -> Option<TagId> {
        self.assigned.get(&instance).copied()
    }

    /// Every (instance, tag) assignment, ascending instance id.
    pub fn assignments(&self) -> impl Iterator<Item = (InstanceId, TagId)> + '_ {
        self.assigned.iter().map(|(i, t)| (*i, *t))
    }

    /// The instances carrying `tag`, ascending.
    pub fn instances_with(&self, tag: TagId) -> Vec<InstanceId> {
        self.assigned
            .iter()
            .filter(|(_, t)| **t == tag)
            .map(|(i, _)| *i)
            .collect()
    }
}

impl Model {
    /// The tag with this id.
    pub fn tag(&self, id: TagId) -> Option<&Tag> {
        self.tags().get(id)
    }

    /// The tag the instance carries; `None` when untagged or unknown.
    pub fn instance_tag(&self, instance: InstanceId) -> Option<TagId> {
        self.tags().instance_tag(instance)
    }

    /// False only when the instance carries a tag that is hidden. Looks at the instance's own
    /// tag, not at the instances that contain it.
    pub fn instance_visible(&self, instance: InstanceId) -> bool {
        let _ = instance;
        unimplemented!("m10-tags-kernel")
    }

    /// The instances of `body` that are not hidden (`instance_visible`), in id order. Empty
    /// when the body doesn't exist. Every walk that draws, picks, snaps, selects or exports
    /// goes through this instead of `Body::instances`; an instance inside a hidden one is
    /// never reached because its parent is skipped.
    pub fn visible_instances(
        &self,
        body: BodyId,
    ) -> impl Iterator<Item = (InstanceId, &Instance)> + '_ {
        self.body(body)
            .into_iter()
            .flat_map(|b| b.instances())
            .filter(|(id, _)| self.instance_visible(*id))
    }

    /// Add a visible tag named `name` as one record; returns its id (never one used before
    /// in this model). Errors: `BadTagName`.
    pub fn add_tag(&mut self, name: &str, log: &mut ModelEdit) -> Result<TagId, ModelError> {
        let _ = (name, log);
        unimplemented!("m10-tags-kernel")
    }

    /// Rename the tag as one record; nothing when the name is unchanged. Errors: `MissingTag`,
    /// `BadTagName`.
    pub fn rename_tag(
        &mut self,
        id: TagId,
        name: &str,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let _ = (id, name, log);
        unimplemented!("m10-tags-kernel")
    }

    /// Show or hide the tag as one record; nothing when unchanged. Errors: `MissingTag`.
    pub fn set_tag_visible(
        &mut self,
        id: TagId,
        visible: bool,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let _ = (id, visible, log);
        unimplemented!("m10-tags-kernel")
    }

    /// Remove the tag: first untag every instance carrying it (ascending instance id), then
    /// remove it. Errors: `MissingTag`.
    pub fn remove_tag(&mut self, id: TagId, log: &mut ModelEdit) -> Result<(), ModelError> {
        let _ = (id, log);
        unimplemented!("m10-tags-kernel")
    }

    /// Give the instance (in any body) the tag, or untag it with `None`, as one record;
    /// nothing when unchanged. Errors: `UnknownInstance`, `MissingTag`.
    pub fn assign_tag(
        &mut self,
        instance: InstanceId,
        tag: Option<TagId>,
        log: &mut ModelEdit,
    ) -> Result<(), ModelError> {
        let _ = (instance, tag, log);
        unimplemented!("m10-tags-kernel")
    }

    /// Apply one tag record (called by `Model::apply`). On error, nothing changes.
    pub(crate) fn apply_tag_record(&mut self, record: &TagRecord) -> Result<(), ModelError> {
        let _ = record;
        unimplemented!("m10-tags-kernel")
    }
}
