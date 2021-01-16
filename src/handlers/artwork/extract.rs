use aspotify::{Album, Artist, Response, Show};

pub trait ExtractArt {
    fn get_art(&self) -> Option<String>;
}

impl<T: ExtractArt> ExtractArt for Response<T> {
    fn get_art(&self) -> Option<String> {
        self.data.get_art()
    }
}

impl ExtractArt for Album {
    fn get_art(&self) -> Option<String> {
        self.images.get(0).map(|i| i.url.clone())
    }
}

impl ExtractArt for Show {
    fn get_art(&self) -> Option<String> {
        self.images.get(0).map(|i| i.url.clone())
    }
}

impl ExtractArt for Artist {
    fn get_art(&self) -> Option<String> {
        self.images.get(0).map(|i| i.url.clone())
    }
}
