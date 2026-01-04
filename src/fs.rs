use anyhow::Result;
use std::path::Path;

/// Abstraction over file system operations for testing
pub trait FileSystem: Send + Sync {
    /// Read file contents as a string
    fn read_to_string(&self, path: &Path) -> Result<String>;

    /// Write string contents to a file
    fn write(&self, path: &Path, contents: &str) -> Result<()>;

    /// Check if a path exists
    fn exists(&self, path: &Path) -> bool;

    /// Check if a path is a file
    fn is_file(&self, path: &Path) -> bool;

    /// Check if a path is a directory
    fn is_dir(&self, path: &Path) -> bool;

    /// Create a directory and all parent directories
    fn create_dir_all(&self, path: &Path) -> Result<()>;

    /// Read directory entries
    fn read_dir(&self, path: &Path) -> Result<Vec<std::path::PathBuf>>;

    /// Rename (atomic move) a file
    fn rename(&self, from: &Path, to: &Path) -> Result<()>;
}

/// Real file system implementation using std::fs
#[derive(Debug, Default, Clone)]
pub struct RealFileSystem;

impl FileSystem for RealFileSystem {
    fn read_to_string(&self, path: &Path) -> Result<String> {
        Ok(std::fs::read_to_string(path)?)
    }

    fn write(&self, path: &Path, contents: &str) -> Result<()> {
        Ok(std::fs::write(path, contents)?)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        Ok(std::fs::create_dir_all(path)?)
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<std::path::PathBuf>> {
        let entries: Result<Vec<_>, _> = std::fs::read_dir(path)?
            .map(|entry| entry.map(|e| e.path()))
            .collect();
        Ok(entries?)
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        Ok(std::fs::rename(from, to)?)
    }
}

/// In-memory file system for testing
#[cfg(test)]
pub mod mock {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};

    #[derive(Debug, Default, Clone)]
    pub struct InMemoryFileSystem {
        files: Arc<RwLock<HashMap<std::path::PathBuf, String>>>,
        directories: Arc<RwLock<std::collections::HashSet<std::path::PathBuf>>>,
    }

    impl InMemoryFileSystem {
        pub fn new() -> Self {
            Self::default()
        }

        /// Add a file to the mock file system
        pub fn add_file(&self, path: impl AsRef<Path>, contents: impl Into<String>) {
            let path = path.as_ref().to_path_buf();
            // Add all parent directories
            if let Some(parent) = path.parent() {
                let mut current = std::path::PathBuf::new();
                for component in parent.components() {
                    current.push(component);
                    self.directories.write().unwrap().insert(current.clone());
                }
            }
            self.files.write().unwrap().insert(path, contents.into());
        }

        /// Get all files (for verification in tests)
        pub fn get_files(&self) -> HashMap<std::path::PathBuf, String> {
            self.files.read().unwrap().clone()
        }
    }

    impl FileSystem for InMemoryFileSystem {
        fn read_to_string(&self, path: &Path) -> Result<String> {
            self.files
                .read()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("File not found: {}", path.display()))
        }

        fn write(&self, path: &Path, contents: &str) -> Result<()> {
            self.files
                .write()
                .unwrap()
                .insert(path.to_path_buf(), contents.to_string());
            Ok(())
        }

        fn exists(&self, path: &Path) -> bool {
            self.files.read().unwrap().contains_key(path)
                || self.directories.read().unwrap().contains(path)
        }

        fn is_file(&self, path: &Path) -> bool {
            self.files.read().unwrap().contains_key(path)
        }

        fn is_dir(&self, path: &Path) -> bool {
            self.directories.read().unwrap().contains(path)
        }

        fn create_dir_all(&self, path: &Path) -> Result<()> {
            let mut current = std::path::PathBuf::new();
            for component in path.components() {
                current.push(component);
                self.directories.write().unwrap().insert(current.clone());
            }
            Ok(())
        }

        fn read_dir(&self, path: &Path) -> Result<Vec<std::path::PathBuf>> {
            let files = self.files.read().unwrap();
            let entries: Vec<_> = files
                .keys()
                .filter(|p| p.parent() == Some(path))
                .cloned()
                .collect();
            Ok(entries)
        }

        fn rename(&self, from: &Path, to: &Path) -> Result<()> {
            let mut files = self.files.write().unwrap();
            if let Some(contents) = files.remove(from) {
                files.insert(to.to_path_buf(), contents);
                Ok(())
            } else {
                Err(anyhow::anyhow!("File not found: {}", from.display()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_real_file_system() {
        let fs = RealFileSystem;
        // Test exists on current directory
        assert!(fs.exists(Path::new(".")));
        assert!(fs.is_dir(Path::new(".")));
    }

    #[test]
    fn test_in_memory_file_system() {
        use mock::InMemoryFileSystem;

        let fs = InMemoryFileSystem::new();

        // Add a file
        fs.add_file("test/file.txt", "Hello, World!");

        // Test exists
        assert!(fs.exists(Path::new("test/file.txt")));
        assert!(fs.is_file(Path::new("test/file.txt")));

        // Test directory exists
        assert!(fs.exists(Path::new("test")));
        assert!(fs.is_dir(Path::new("test")));

        // Test read
        assert_eq!(
            fs.read_to_string(Path::new("test/file.txt")).unwrap(),
            "Hello, World!"
        );

        // Test write
        fs.write(Path::new("test/new.txt"), "New content").unwrap();
        assert_eq!(
            fs.read_to_string(Path::new("test/new.txt")).unwrap(),
            "New content"
        );

        // Test read_dir
        let entries = fs.read_dir(Path::new("test")).unwrap();
        assert_eq!(entries.len(), 2);
    }
}
