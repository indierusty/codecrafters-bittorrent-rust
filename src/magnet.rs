use std::collections::HashMap;

use anyhow::Context;
use reqwest::Url;

use crate::torrent::TorrentInfo;

pub struct Magnet {
    pub tracker_url: String,
    /// The name of the file to be downloaded
    pub name: String,
    /// 40-char hex-encoded info hash
    pub info_hash: [u8; 20],
}

impl TorrentInfo for Magnet {
    fn announce(&self) -> String {
        self.tracker_url.to_owned()
    }

    fn info_hash(&self) -> [u8; 20] {
        self.info_hash
    }

    fn length(&self) -> u32 {
        1
    }
}

impl Magnet {
    pub fn parse(uri: &str) -> anyhow::Result<Magnet> {
        let url = Url::parse(&uri).unwrap();
        let pairs = url.query_pairs().fold(HashMap::new(), |mut acc, q| {
            acc.insert(q.0, q.1);
            acc
        });

        let tracker_url = pairs
            .get("tr")
            .context("magnet-link doesn't have tracker url")?
            .to_string();
        let name = pairs
            .get("dn")
            .context("magnet-link doesn't have name")?
            .to_string();
        let info_hash = pairs
            .get("xt")
            .context("magnet-link doesn't have info hash")?
            .split(':')
            .last()
            .unwrap()
            .to_string();
        let info_hash = hex::decode(info_hash)?;
        let info_hash: [u8; 20] = std::array::from_fn(|i| info_hash[i]);

        Ok(Magnet {
            tracker_url,
            name,
            info_hash,
        })
    }
}
