//! Which messages are turns, where they are anchored, and the request they produce.

use serenity::all::{ChannelId, ChannelType, GuildId, MessageFlags, UserId};

use crate::access::message::images;
use crate::access::route::{
    Arrival, Arrived, Place, Trigger, chat_request, press_visibility, serves, thread_parent,
    trigger,
};
use crate::core::types::Visibility;

#[test]
fn only_threads_inherit_the_allowlist_of_their_parent() {
    let category = ChannelId::new(10);
    let channel = ChannelId::new(11);
    let allow = [category];

    let text_parent = thread_parent(Some(ChannelType::Text), Some(category));
    assert_eq!(text_parent, None, "a category is not a thread parent");
    assert!(!serves(&allow, channel, text_parent));

    let thread_parent_id = thread_parent(Some(ChannelType::PublicThread), Some(category));
    assert_eq!(thread_parent_id, Some(category));
    assert!(serves(&allow, ChannelId::new(12), thread_parent_id));

    assert!(serves(&[], channel, None), "an empty allowlist admits all");
    assert!(serves(&[channel], channel, None));
    assert_eq!(thread_parent(None, Some(category)), None);
}

#[test]
fn an_ephemeral_press_confirms_privately_and_stays_ephemeral() {
    assert_eq!(
        press_visibility(Some(MessageFlags::EPHEMERAL)),
        (Visibility::Private, true)
    );
    assert_eq!(
        press_visibility(Some(MessageFlags::empty())),
        (Visibility::Public, false)
    );
    assert_eq!(press_visibility(None), (Visibility::Public, false));
}

#[test]
fn each_place_sets_visibility_and_continuation() {
    let at = ChannelId::new(5);
    let make = |place| {
        chat_request(
            place,
            UserId::new(1),
            GuildId::new(2),
            vec![],
            "q".into(),
            None,
            Vec::new(),
        )
    };

    let private = make(Place::Private(at));
    assert_eq!(private.visibility, Visibility::Private);
    assert!(private.continue_channel);

    let thread = make(Place::Thread(at));
    assert_eq!(thread.visibility, Visibility::Public);
    assert!(thread.continue_channel);
    assert_eq!(thread.channel_id, "5");
    assert!(thread.conversation_id.is_none());

    let inline = make(Place::Inline(at));
    assert_eq!(inline.visibility, Visibility::Public);
    assert!(!inline.continue_channel);

    let wire = serde_json::to_value(&private).unwrap_or_default();
    assert_eq!(wire["visibility"], "private");
    assert_eq!(wire["continue_channel"], true);
}

#[test]
fn a_thread_answers_a_reply_and_stays_quiet_for_everything_else() {
    let in_thread = |mentions_bot, replies_to_bot| {
        trigger(Arrival {
            at: Arrived::Thread,
            mentions_bot,
            replies_to_bot,
        })
    };

    assert_eq!(in_thread(false, true), Some(Trigger::Reply));
    assert_eq!(in_thread(true, true), Some(Trigger::Reply));
    // People talk in the thread without the bot answering every line.
    assert_eq!(in_thread(false, false), None);
    // A mention alone is not the reply the thread continues on.
    assert_eq!(in_thread(true, false), None);
}

#[test]
fn addressing_the_bot_in_a_channel_opens_a_thread() {
    let in_channel = |mentions_bot, replies_to_bot| {
        trigger(Arrival {
            at: Arrived::Channel,
            mentions_bot,
            replies_to_bot,
        })
    };

    assert_eq!(in_channel(true, false), Some(Trigger::Opening));
    assert_eq!(in_channel(false, true), Some(Trigger::Opening));
    assert_eq!(in_channel(false, false), None);
}

#[test]
fn every_direct_message_is_a_turn() {
    assert_eq!(
        trigger(Arrival {
            at: Arrived::Direct,
            ..Arrival::default()
        }),
        Some(Trigger::Direct)
    );
}

#[test]
fn a_direct_message_is_private_so_personal_memory_applies_and_a_thread_is_public() {
    let make = |place| {
        chat_request(
            place,
            UserId::new(1),
            GuildId::new(2),
            vec![],
            "q".into(),
            None,
            Vec::new(),
        )
    };

    // Memory recall and the memory tools key on this, so a DM is the only place they apply.
    assert_eq!(
        make(Place::Private(ChannelId::new(7))).visibility,
        Visibility::Private
    );
    assert_eq!(
        make(Place::Thread(ChannelId::new(7))).visibility,
        Visibility::Public
    );
}

#[test]
fn the_quoted_message_rides_the_request_and_is_left_out_when_there_is_none() {
    let make = |reply_to| {
        chat_request(
            Place::Thread(ChannelId::new(5)),
            UserId::new(1),
            GuildId::new(2),
            vec![],
            "and on Sunday?".into(),
            reply_to,
            Vec::new(),
        )
    };

    let quoted = make(Some("The library closes at 10pm.".to_owned()));
    let wire = serde_json::to_value(&quoted).unwrap_or_default();
    assert_eq!(wire["reply_to"], "The library closes at 10pm.");

    let plain = serde_json::to_value(make(None)).unwrap_or_default();
    assert!(plain.get("reply_to").is_none());
}

#[test]
fn a_message_with_only_an_image_is_still_a_question() {
    let attached = images([("https://cdn/one.png", Some("image/png"))], 4);
    let req = chat_request(
        Place::Thread(ChannelId::new(5)),
        UserId::new(1),
        GuildId::new(2),
        vec![],
        String::new(),
        None,
        attached,
    );
    let wire = serde_json::to_value(&req).unwrap_or_default();
    assert_eq!(wire["images"][0]["url"], "https://cdn/one.png");
    assert_eq!(wire["images"][0]["media_type"], "image/png");

    let none = chat_request(
        Place::Thread(ChannelId::new(5)),
        UserId::new(1),
        GuildId::new(2),
        vec![],
        "q".into(),
        None,
        Vec::new(),
    );
    let wire = serde_json::to_value(&none).unwrap_or_default();
    assert!(wire.get("images").is_none(), "no images writes no field");
}
