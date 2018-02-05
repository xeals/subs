#[derive(Debug)]
pub struct Queue {
    songs: Vec<usize>,
    position: usize,
}

impl Queue {
    pub fn new() -> Queue {
        Queue {
            songs: Vec::new(),
            position: 0,
        }
    }

    pub fn len(&self) -> usize { self.songs.len() }

    pub fn is_empty(&self) -> bool { self.len() == 0 }

    pub fn append(&mut self, song: usize) { self.songs.push(song); }

    pub fn insert_next(&mut self, song: usize) {
        self.songs.insert(self.position + 1, song);
    }

    pub fn has_next(&self) -> bool {
        // will have next if len is 3 and position is 1
        (self.len() == 1 && self.position == 0)
            || (self.len() - 2 >= self.position)
    }

    pub fn next(&mut self) -> Option<usize> {
        if !self.is_empty() && self.position <= self.len() - 1 {
            let song = self.songs[self.position];
            self.position += 1;
            Some(song)
        } else {
            None
        }
    }
}

impl ::std::iter::Extend<usize> for Queue {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = usize>,
    {
        self.songs.extend(iter);
    }
}
