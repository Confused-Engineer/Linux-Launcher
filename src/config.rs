pub(crate) struct ApplicationEntries
{
    configfile: ini::Ini,
}

impl ApplicationEntries
{
    pub fn new() -> Self
    {
        Self { 
            configfile: ini::Ini::load_from_file_noescape(Self::current_file()).unwrap_or(ini::Ini::new())
        }
    }

    pub fn is_empty(&mut self) -> bool
    {
        if self.configfile.len() == 1
        {
            return true;
        }
        false
    }

    pub fn reset(&mut self) -> Self
    {
        let currentpath = Self::current_file();
        let path = std::path::Path::new(&currentpath);
        if path.exists()
        {
            let _ = std::fs::remove_file(&currentpath);
        }

        let _ = std::fs::write(currentpath, include_str!("../assets/config.ini"));
        Self::new()
    }

    pub fn return_config(&mut self) -> ini::Ini
    {
        self.configfile.clone()
    }

    pub fn current_file() -> String
    {
        let exe_name = std::env::current_exe().unwrap().display().to_string().split("\\").last().unwrap().to_string();
        let path = std::env::current_exe().unwrap().display().to_string().replace(&exe_name, "LL.ini");
        return path;
    }


    
}

