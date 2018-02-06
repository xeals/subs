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

    pub fn position(&self) -> usize { self.position }

    pub fn len(&self) -> usize { self.songs.len() }

    pub fn is_empty(&self) -> bool { self.len() == 0 }

    pub fn append(&mut self, song: usize) { self.songs.push(song); }

    pub fn clear(&mut self) { self.songs.clear() }

    pub fn insert_next(&mut self, song: usize) {
        if self.position == self.len() {
            self.songs.push(song)
        } else {
            self.songs.insert(self.position + 1, song);
        }
    }

    pub fn current(&self) -> Option<usize> {
        self.songs.get(self.position).map(|i| *i)
    }

    pub fn has_next(&self) -> bool {
        // will have next if len is 3 and position is 1
        (self.len() == 1 && self.position == 0)
            || (self.len() - 2 >= self.position)
    }

    pub fn next(&mut self) -> Option<usize> {
        if !self.is_empty() && self.position <= self.len() - 1 {
            self.position += 1;
            Some(self.songs[self.position])
        } else {
            None
        }
    }

    pub fn prev(&mut self) -> Option<usize> {
        if !self.is_empty() {
            if self.position == 0 {
            } else {
                self.position -= 1;
            }
            Some(self.songs[self.position])
        } else {
            None
        }
    }

    pub fn prev2(&mut self) -> Option<usize> {
        self.prev();
        self.prev()
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
