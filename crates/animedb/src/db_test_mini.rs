//! Mini test module to isolate the episode insert issue
#[cfg(test)]
mod mini_episode_test {
    use crate::{AnimeDb, CanonicalEpisode, CanonicalMedia, MediaKind, SourceName};

    #[test]
    fn mini_episode_insert() {
        use crate::model::ExternalId;
        
        let mut db = AnimeDb::open_in_memory().unwrap();
        
        let media = CanonicalMedia {
            media_kind: MediaKind::Anime,
            title_display: "Test".into(),
            title_romaji: None,
            title_english: None,
            title_native: None,
            synopsis: None,
            format: None,
            status: None,
            season: None,
            season_year: None,
            episodes: None,
            chapters: None,
            volumes: None,
            country_of_origin: None,
            cover_image: None,
            banner_image: None,
            provider_rating: None,
            nsfw: false,
            aliases: vec![],
            genres: vec![],
            tags: vec![],
            external_ids: vec![ExternalId {
                source: SourceName::Kitsu,
                source_id: "1".into(),
                url: None,
            }],
            source_payloads: vec![],
            field_provenance: vec![],
        };
        
        let media_id = db.upsert_media(&media).unwrap();
        
        let episode = CanonicalEpisode {
            source: SourceName::Kitsu,
            source_id: "ep1".into(),
            media_kind: MediaKind::Anime,
            season_number: Some(1),
            episode_number: Some(1),
            absolute_number: Some(1),
            title_display: Some("The Hospital".into()),
            title_original: Some("Byouin".into()),
            synopsis: Some("Tenma operates on a young boy.".into()),
            air_date: Some("2005-04-05".into()),
            runtime_minutes: Some(23),
            thumbnail_url: None,
            raw_titles_json: None,
            raw_json: None,
        };
        
        println!("About to call upsert_episode...");
        let result = db.upsert_episode(&episode, media_id);
        println!("Result: {:?}", result);
        
        assert!(result.is_ok());
    }
}
