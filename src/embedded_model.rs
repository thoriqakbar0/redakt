pub(crate) struct EmbeddedModel {
    pub(crate) rules: &'static [u8],
    pub(crate) tagger: &'static [u8],
    pub(crate) cues: &'static [u8],
}

impl EmbeddedModel {
    pub(crate) fn load() -> Result<Self, String> {
        let bundle = include_bytes!("../models/redakt.bin");
        let (magic, lengths) = bundle
            .split_first_chunk::<4>()
            .ok_or("Truncated model bundle header")?;
        if magic != b"RDM1" {
            return Err("Unsupported model bundle format".into());
        }

        let (rules_length, lengths) = lengths
            .split_first_chunk()
            .ok_or("Missing rules section length")?;

        let (tagger_length, payload) = lengths
            .split_first_chunk()
            .ok_or("Missing tagger section length")?;

        let rules_length = u32::from_le_bytes(*rules_length) as usize;
        let tagger_length = u32::from_le_bytes(*tagger_length) as usize;

        let (rules, learned) = payload
            .split_at_checked(rules_length)
            .ok_or("Truncated rules section")?;
        let (tagger, cues) = learned
            .split_at_checked(tagger_length)
            .ok_or("Truncated tagger section")?;
        Ok(Self {
            rules,
            tagger,
            cues,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_match_standalone_weights() {
        let model = EmbeddedModel::load().unwrap();
        assert_eq!(
            model.rules,
            include_bytes!("../models/weights.bin").as_slice()
        );
    }
}
