#![allow(clippy::unwrap_used)] // Test helpers panic on setup failure by design.

mod common;

use common::{open, request, setup};

#[test]
fn timeline_pages_newest_first_and_numbers_repeats() {
    let s = setup();
    let mut d = open(&s);
    let first = d.save(request("new", &[], &["sqlite"])).unwrap();
    let other = d.save(request("new", &[], &["rust"])).unwrap();
    let again = d.save(request(&first.topic_id, &["sqlite"], &[])).unwrap();

    let page = d.index.timeline(None, None, 2).unwrap();
    let ids: Vec<&str> = page.items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, [again.note_id.as_str(), other.note_id.as_str()]);
    assert_eq!((page.items[0].ask_index, page.items[0].ask_count), (2, 2));
    assert!(page.items[0].question.contains("sqlite-vec"));

    let rest = d.index.timeline(None, page.next.as_deref(), 2).unwrap();
    assert_eq!(rest.items.len(), 1);
    assert_eq!(rest.items[0].id, first.note_id);
    assert_eq!(rest.items[0].ask_index, 1);
    assert!(rest.next.is_none());

    let tagged = d.index.timeline(Some("rust"), None, 10).unwrap();
    assert_eq!(tagged.items.len(), 1);
    assert_eq!(tagged.items[0].id, other.note_id);
}

#[test]
fn stats_count_days_agents_and_annotated_repeats() {
    let s = setup();
    let mut d = open(&s);
    let first = d.save(request("new", &[], &["sqlite"])).unwrap();
    d.save(request(&first.topic_id, &["sqlite"], &[])).unwrap();
    let stats = d.index.stats().unwrap();
    assert_eq!(stats.by_day.iter().map(|x| x.notes).sum::<usize>(), 2);
    assert_eq!(stats.by_agent.len(), 1);
    assert_eq!(stats.by_agent[0].notes, 2);
    assert_eq!(stats.annotated_repeated_topics, 0);

    d.annotate(&first.note_id, "懂了").unwrap();
    assert_eq!(d.index.stats().unwrap().annotated_repeated_topics, 1);
}
