// Copyright (c) 2017, 2018, 2023 Marek Benc <benc.marek.elektro98@proton.me>
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
//

use home;
use log::{info, warn, error};

use std::env;
use std::fmt;
use std::path;
use std::fs;
use std::io;
use std::num;
use std::io::prelude::*;

use trs80m1_rs_core::cassette; // For cassette::Format.


// Names for determining where to find the configuration folder and files:
const WINDOWS_DEV_NAME:      &'static str = "DusXMT";
const WINDOWS_PROJ_NAME:     &'static str = "trs80m1-rs";
const UNIX_HIDDEN_DIR_NAME:  &'static str = ".trs80m1-rs";
const CONFIG_FILE_NAME:      &'static str = "config.ini";

// Configuration items.
//
// This is data that represents what's in the configuration file, accessible to
// all parts of the program via the configuration system.
//
#[derive(Debug)]
pub struct ConfigItems {

    // [General] Entries:
    pub general_level_1_rom:             Option<String>,
    pub general_level_2_rom:             Option<String>,
    pub general_misc_rom:                Option<String>,

    pub general_default_rom:             u32,
    pub general_ram_size:                u32,


    // [Keyboard] Entries:
    pub keyboard_ms_per_keypress:        u32,


    // [Video] Entries:
    pub video_windowed_resolution:       (u32, u32),
    pub video_fullscreen_resolution:     (u32, u32),

    pub video_bg_color:                  (u8, u8, u8),
    pub video_fg_color:                  (u8, u8, u8),

    pub video_desktop_fullscreen_mode:   bool,
    pub video_use_hw_accel:              bool,
    pub video_use_vsync:                 bool,

    pub video_character_generator:       u32,
    pub video_lowercase_mod:             bool,


    // [Cassette] Entries:
    pub cassette_file:                   Option<String>,
    pub cassette_file_format:            cassette::Format,
    pub cassette_file_offset:            usize,
}

impl ConfigItems {
    // The values in a new ConfigItems structure are defined, but not useful.
    //
    // The fields should be filled in by the adequate values from the config
    // file text on initialization of the ConfigSystem.
    //
    fn new_uninitialized() -> ConfigItems {

        ConfigItems {
            general_level_1_rom:             None,
            general_level_2_rom:             None,
            general_misc_rom:                None,

            general_default_rom:             0,
            general_ram_size:                0,

            keyboard_ms_per_keypress:        0,

            video_windowed_resolution:       (0, 0),
            video_fullscreen_resolution:     (0, 0),

            video_bg_color:                  (0, 0, 0),
            video_fg_color:                  (0, 0, 0),

            video_desktop_fullscreen_mode:   false,
            video_use_hw_accel:              false,
            video_use_vsync:                 false,

            video_character_generator:       0,
            video_lowercase_mod:             false,

            cassette_file:                   None,
            cassette_file_format:            cassette::Format::CAS,
            cassette_file_offset:            0,
        }
    }
}

// Information source.
//
// The point of this enum is to allow the use of configuration information both
// from the configuration file, and from other parts of the program.
//
// The main distinction between the two source types, and the motivation for
// introducing this enum, is to have descriptive error messages for both
// use cases.
//
#[derive(Debug)]
pub enum ConfigInfoSource {
    ConfigFile { line_number: usize, line_text: String },
    ExternalSource { section_name: String, entry_name: String, invocation_text: String },
}

impl ConfigInfoSource {
    fn from_config_file(line_number: usize, line_text: &str) -> ConfigInfoSource {
        ConfigInfoSource::ConfigFile {
            line_number,
            line_text:   line_text.to_owned(),
        }
    }
    fn from_external_source(section_name: &str, entry_name: &str, invocation_text: &str) -> ConfigInfoSource {
        ConfigInfoSource::ExternalSource {
            section_name:    section_name.to_owned(),
            entry_name:      entry_name.to_owned(),
            invocation_text: invocation_text.to_owned(),
        }
    }
    fn error_prefix(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ConfigInfoSource::ConfigFile { line_number, ref line_text } => {
                write!(f, "error on line {}, `{}', ", line_number + 1, line_text)
            },
            ConfigInfoSource::ExternalSource { ref section_name, ref entry_name, ref invocation_text } => {
                write!(f, "invalid new value `{}' for the config entry `{}' of the `[{}]' section: ", retrieve_entry_assignee(invocation_text).as_str(), entry_name, section_name)
            },
        }
    }
    fn argument_text(&self) -> String {
        match *self {
            ConfigInfoSource::ConfigFile { ref line_text, .. } => {
                retrieve_entry_assignee(line_text)
            },
            ConfigInfoSource::ExternalSource { ref invocation_text, .. } => {
                retrieve_entry_assignee(invocation_text)
            },
        }
    }
}

// Error structure used within the module:
#[derive(Debug)]
pub enum ConfigError {
    RedundantSection(String, usize, usize),
    RedundantEntry(String, String, usize, usize),
    EntryIntParsingError(ConfigInfoSource, num::ParseIntError),
    TextAfterConfigSectionClosingBracket(ConfigInfoSource),
    NonAlphaCharactersInConfigSectionName(ConfigInfoSource),
    ClosingBracketMissingInConfigSectionHeader(ConfigInfoSource),
    EntryNameBeginsWithNonAlpha(ConfigInfoSource),
    EntryNameContainsSpaces(ConfigInfoSource),
    EntryNameContainsNonAlnumChars(ConfigInfoSource),
    EntryContainsSeveralEqualsSigns(ConfigInfoSource),
    EntryEqualsSignMissing(ConfigInfoSource),
    EntryArgumentMissing(ConfigInfoSource),
    InvalidResolutionSpecifier(ConfigInfoSource),
    InvalidColorSpecifier(ConfigInfoSource),
    InvalidBoolSpecifier(ConfigInfoSource),
    InvalidCassetteFormatSpecifier(ConfigInfoSource),
    InvalidRamSpecifier(ConfigInfoSource),
    TooMuchRamRequested(ConfigInfoSource, u32),
    DefaultRomOutOfRange(ConfigInfoSource, u32),
    CharacterGeneratorOutOfRange(ConfigInfoSource, u32),
    EntrySpecNoSectionNameSpecified(String),
    EntrySpecNoEntryNameSpecified(String),
    EntrySpecNoSuchConfigEntry(String),
    IoError(io::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ConfigError::RedundantSection(ref section_name, first, second) => {
                write!(f, "section `[{}]' is present in the configuration file more than once, on line {} and line {}", section_name, first + 1, second + 1)
            },
            ConfigError::RedundantEntry(ref section_name, ref entry_name, first, second) => {
                write!(f, "entry `{}' is present more than once in the `[{}]' section of the configuration file, on line {} and line {}", entry_name, section_name, first + 1, second + 1)
            },
            ConfigError::EntryIntParsingError(ref info_source, ref inner_error) => {
                info_source.error_prefix(f)?;
                write!(f, "failed to parse the entry argument: {}", inner_error)
            },
            ConfigError::TextAfterConfigSectionClosingBracket(ref info_source) => {
                info_source.error_prefix(f)?;
                write!(f, "there is text present after the config section header's closing bracket")
            },
            ConfigError::NonAlphaCharactersInConfigSectionName(ref info_source) => {
                info_source.error_prefix(f)?;
                write!(f, "the config section name contains non-alphabetical characters")
            },
            ConfigError::ClosingBracketMissingInConfigSectionHeader(ref info_source) => {
                info_source.error_prefix(f)?;
                write!(f, "the closing bracket is missing in the config section header")
            },
            ConfigError::EntryNameBeginsWithNonAlpha(ref info_source) => {
                info_source.error_prefix(f)?;
                write!(f, "the entry's name begins with a non-alphabetical character")
            },
            ConfigError::EntryNameContainsSpaces(ref info_source) => {
                info_source.error_prefix(f)?;
                write!(f, "the entry's name contains spaces")
            },
            ConfigError::EntryNameContainsNonAlnumChars(ref info_source) => {
                info_source.error_prefix(f)?;
                write!(f, "the entry's name contains non-alphanumerical characters")
            },
            ConfigError::EntryContainsSeveralEqualsSigns(ref info_source) => {
                info_source.error_prefix(f)?;
                write!(f, "the entry contains several equals signs")
            },
            ConfigError::EntryEqualsSignMissing(ref info_source) => {
                info_source.error_prefix(f)?;
                write!(f, "the entry doesn't contain an equals sign")
            },
            ConfigError::EntryArgumentMissing(ref info_source) => {
                info_source.error_prefix(f)?;
                write!(f, "the entry doesn't contain an argument")
            },
            ConfigError::InvalidResolutionSpecifier(ref info_source) => {
                info_source.error_prefix(f)?;
                write!(f, "invalid resolution specification")
            },
            ConfigError::InvalidColorSpecifier(ref info_source) => {
                info_source.error_prefix(f)?;
                write!(f, "invalid color specification")
            },
            ConfigError::InvalidBoolSpecifier(ref info_source) => {
                info_source.error_prefix(f)?;
                write!(f, "invalid boolean specification")
            },
            ConfigError::InvalidCassetteFormatSpecifier(ref info_source) => {
                info_source.error_prefix(f)?;
                write!(f, "invalid cassette format specification, please use either CAS or CPT")
            },
            ConfigError::InvalidRamSpecifier(ref info_source) => {
                info_source.error_prefix(f)?;
                write!(f, "invalid ram specification")
            },
            ConfigError::TooMuchRamRequested(ref info_source, ram_requested) => {
                info_source.error_prefix(f)?;
                if (ram_requested % 1024) == 0 {
                    write!(f, "the requested amout of ram ({}K) is more than what can be installed in the machine, it supports only up to 48K (49152 bytes) of ram", ram_requested / 1024)
                } else {
                    write!(f, "the requested amount of ram ({} bytes) is more than what can be installed in the machine, it supports only up to 48K (49152 bytes) of ram", ram_requested)
                }
            }
            ConfigError::DefaultRomOutOfRange(ref info_source, selection) => {
                info_source.error_prefix(f)?;
                write!(f, "the specified default rom selection of {} is out of range, please choose either 1 (level 1 basic), 2 (level 2 basic), or 3 (miscellaneous rom)", selection)
            },
            ConfigError::CharacterGeneratorOutOfRange(ref info_source, selection) => {
                info_source.error_prefix(f)?;
                write!(f, "the specified character generator selection of {} is out of range, please choose from 1 to 3", selection)
            },
            ConfigError::EntrySpecNoSectionNameSpecified(ref entry_specifier) => {
                write!(f, "invalid entry specifier `{}': no section name specified", entry_specifier)
            },
            ConfigError::EntrySpecNoEntryNameSpecified(ref entry_specifier) => {
                write!(f, "invalid entry specifier `{}': no entry name specified", entry_specifier)
            },
            ConfigError::EntrySpecNoSuchConfigEntry(ref entry_specifier) => {
                write!(f, "entry specifier `{}' does not correspond to any config entry", entry_specifier)
            },
            ConfigError::IoError(ref io_error) => {
                io_error.fmt(f)
            },
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(io_error: io::Error) -> ConfigError {
        ConfigError::IoError(io_error)
    }
}

// Apply action.
//
// The following enum describes the actions that need to be taken in order for
// the new value of an entry after doing a configuration change to be utilized.
//
#[derive(Debug, Copy, Clone)]
pub enum ConfigChangeApplyAction {
    RomChange(u32),
    ChangeRamSize,
    UpdateMsPerKeypress,
    ChangeWindowedResolution,
    ChangeFullscreenResolution,
    ChangeColor,
    ChangeHwAccelUsage,
    ChangeVsyncUsage,
    ChangeCharacterGenerator,
    ChangeLowercaseModUsage,
    UpdateCassetteFile,
    UpdateCassetteFileFormat,
    UpdateCassetteFileOffset,
    UpdateDefaultRomSelection,
    AlreadyUpToDate,
}

// A configuration entry handler:
struct ConfigEntry {

    // The name of the entry:
    entry_name:   String,

    // The default text of the entry, including comments:
    default_text: Box<[String]>,

    // The action to be performed to apply a change of the entry:
    apply_action: ConfigChangeApplyAction,

    // Return an up-to-date version of the entry line, or none if already ok:
    update_line: fn(ConfigInfoSource, &mut ConfigItems) -> Option<String>,

    // Parse the entry line and update the in-memory representation:
    parse_entry: fn(ConfigInfoSource, &mut ConfigItems) -> Result<(), ConfigError>,
}

// Representation of a section:
struct ConfigSection {
    section_name:      String,
    entries:           Box<[ConfigEntry]>,
    obsolete_entries:  Box<[String]>,
}

// The configuration system structure:
pub struct ConfigSystem {
    pub config_dir_path:  path::PathBuf,
    config_file_path:     path::PathBuf,

    pub config_items:     ConfigItems,
    conf_file_lines:      Vec<String>,

    config_sections:      Box<[ConfigSection]>,
}

impl ConfigSystem {
    pub fn new<P: AsRef<path::Path>>(config_dir_in: P) -> Option<ConfigSystem> {
        let config_dir = config_dir_in.as_ref() as &path::Path;

        if check_config_dir(config_dir) {
            let mut config_file_path = config_dir.to_owned();
            config_file_path.push(CONFIG_FILE_NAME);
            let config_file_path = config_file_path;

            let conf_file_lines = match load_config_file(&config_file_path) {
                Ok(lines) => {
                    lines
                },
                Err(error) => {
                    error!("Failed to load the configuration file `{}': {}.", config_file_path.display(), error);

                    return None;
                },
            };
            let mut new_system = ConfigSystem {
                config_dir_path:  config_dir.to_owned(),
                config_file_path,

                config_items:     ConfigItems::new_uninitialized(),
                conf_file_lines,

                config_sections:  new_config_sections(),
            };

            match new_system.sanity_check() {
                Ok(()) => {
                    match new_system.reload_all_sections() {
                        Ok(()) => {
                            info!("Configuration file parsed successfully.");

                            match new_system.write_config_file() {
                                Ok(()) => {
                                    info!("Configuration file updated successfully.");
                                },
                                Err(error) => {
                                    error!("Failed to update the configuration file: {}.", error);
                                },
                            }

                            let _ = new_system.check_obsolete_entries(&new_system.config_file_path);
                            Some(new_system)
                        },
                        Err(error) => {
                            error!("Failed to parse the configuration file: {}.", error);

                            None
                        },
                    }
                },
                Err(error) => {
                    error!("Failed to parse the configuration file: {}.", error);

                    None
                },
            }
        } else {
            None
        }
    }
    fn sanity_check(&self) -> Result<(), ConfigError> {
        for line_iter in 0..self.conf_file_lines.len() {
            if self.conf_file_lines[line_iter].len() != 0 {
                let mut chars = self.conf_file_lines[line_iter].chars();
                let first_char = chars.next();

                match first_char {
                    Some(first_char_ch) => {
                        match first_char_ch {
                            // If it's a comment, it's okay.
                            ';' => { },
                            '#' => { },

                            // If it's a section header, it must have a closing
                            // bracket, and no space after it.
                            '[' => {
                                let mut found_closing_bracket = false;
                                for current_char_ch in chars {
                                    if found_closing_bracket {
                                        return Err(ConfigError::TextAfterConfigSectionClosingBracket(ConfigInfoSource::from_config_file(line_iter, &self.conf_file_lines[line_iter])));
                                    } else if current_char_ch == ']' {
                                        found_closing_bracket = true;
                                    } else if !current_char_ch.is_alphabetic() && !current_char_ch.is_whitespace() {
                                        return Err(ConfigError::NonAlphaCharactersInConfigSectionName(ConfigInfoSource::from_config_file(line_iter, &self.conf_file_lines[line_iter])));
                                    }
                                }
                                if !found_closing_bracket {
                                    return Err(ConfigError::ClosingBracketMissingInConfigSectionHeader(ConfigInfoSource::from_config_file(line_iter, &self.conf_file_lines[line_iter])));
                                }
                            },

                            // If it's an entry, it must contain one word,
                            // followed by a single equals sign, followed by
                            // some argument text.
                            _ => {
                                let mut found_equals = false;
                                let mut found_argument = false;
                                let mut found_space_before_equals = false;

                                if !first_char_ch.is_alphabetic() {
                                    return Err(ConfigError::EntryNameBeginsWithNonAlpha(ConfigInfoSource::from_config_file(line_iter, &self.conf_file_lines[line_iter])));
                                }
                                for current_char_ch in chars {
                                    if !found_equals {
                                        if current_char_ch.is_whitespace() {
                                            found_space_before_equals = true;
                                        } else if current_char_ch.is_alphanumeric() || current_char_ch == '_' {
                                            if found_space_before_equals {
                                                return Err(ConfigError::EntryNameContainsSpaces(ConfigInfoSource::from_config_file(line_iter, &self.conf_file_lines[line_iter])));
                                            }
                                        } else if current_char_ch == '=' {
                                            found_equals = true;
                                        } else {
                                            return Err(ConfigError::EntryNameContainsNonAlnumChars(ConfigInfoSource::from_config_file(line_iter, &self.conf_file_lines[line_iter])));
                                        }
                                    } else {
                                        if current_char_ch == '=' {
                                            return Err(ConfigError::EntryContainsSeveralEqualsSigns(ConfigInfoSource::from_config_file(line_iter, &self.conf_file_lines[line_iter])));
                                        } else if !found_argument && !current_char_ch.is_whitespace() {
                                            found_argument = true;
                                        }
                                    }
                                }
                                if !found_equals {
                                    return Err(ConfigError::EntryEqualsSignMissing(ConfigInfoSource::from_config_file(line_iter, &self.conf_file_lines[line_iter])));
                                }
                                if !found_argument {
                                    return Err(ConfigError::EntryArgumentMissing(ConfigInfoSource::from_config_file(line_iter, &self.conf_file_lines[line_iter])));
                                }
                            },
                        }
                    },
                    None => { },
                }
            }
        }

        Ok(())
    }
    fn reload_all_sections(&mut self) -> Result<(), ConfigError> {
        for section_iter in 0..self.config_sections.len() {

            // Find the section's location:
            let start_index;
            let mut end_index;

            match self.find_section(&self.config_sections[section_iter].section_name)? {
                Some((found_start_index, found_end_index)) => {
                    start_index = found_start_index;
                    end_index   = found_end_index;
                },
                None => {
                    // The section isn't in the configuration file, add it:
                    if self.conf_file_lines.len() != 0 {
                        self.conf_file_lines.push("".to_owned());
                    }
                    let loc = self.conf_file_lines.len();
                    self.conf_file_lines.push(format!("[{}]", self.config_sections[section_iter].section_name));

                    start_index = loc;
                    end_index   = loc;
                }
            }

            // Find all of its entries and reload them:
            for entry_iter in 0..self.config_sections[section_iter].entries.len() {
                // Find the entry's location:
                let entry_loc = match self.find_entry(&self.config_sections[section_iter].section_name, &self.config_sections[section_iter].entries[entry_iter].entry_name, start_index, end_index)? {
                    Some(loc) => { loc },
                    None => {
                        // The entry doesn't exist yet, add it:
                        let mut src_line_iter  = 0;
                        let mut dest_line_iter = end_index + 1;
                        while src_line_iter < self.config_sections[section_iter].entries[entry_iter].default_text.len() {
                            self.conf_file_lines.insert(dest_line_iter, self.config_sections[section_iter].entries[entry_iter].default_text[src_line_iter].to_owned());
                            src_line_iter += 1;
                            dest_line_iter += 1;
                        }
                        end_index = dest_line_iter - 1;
                        self.find_entry(&self.config_sections[section_iter].section_name, &self.config_sections[section_iter].entries[entry_iter].entry_name, start_index, end_index)?.expect(format!(".expect() call: Unable to find the freshly added `{}' entry in the `[{}]' section", self.config_sections[section_iter].entries[entry_iter].entry_name, self.config_sections[section_iter].section_name).as_str())
                    },
                };
                (self.config_sections[section_iter].entries[entry_iter].parse_entry)(ConfigInfoSource::from_config_file(entry_loc, &self.conf_file_lines[entry_loc]), &mut self.config_items)?;
            }
        }

        Ok(())
    }
    fn find_section(&self, section_name: &str) -> Result<Option<(usize, usize)>, ConfigError> {
        let mut already_found  = false;
        let mut in_the_section = false;

        let mut start_index: usize = 0;
        let mut end_index:   usize = 0;

        let compare_str = format!("[{}]", section_name.to_uppercase());

        for line_iter in 0..self.conf_file_lines.len() {
            if self.conf_file_lines[line_iter].to_uppercase() == compare_str {
                if !already_found {
                    already_found = true;
                    in_the_section = true;
                    start_index = line_iter;
                } else {
                    return Err(ConfigError::RedundantSection(section_name.to_owned(), start_index, line_iter));
                }
            } else if in_the_section {
                if let Some(character) = self.conf_file_lines[line_iter].chars().next() {
                    if character == '[' {
                        // Is there a blank line before the next section starts?
                        if (line_iter - 1) > start_index && self.conf_file_lines[line_iter - 1] == "" {
                            end_index = line_iter - 2;
                        } else {
                            end_index = line_iter - 1;
                        }
                        in_the_section = false;
                    }
                }
            }
        }
        // If we haven't encountered the end of the section, that means that
        // the section extends to the end of the file.
        if in_the_section {
            end_index = self.conf_file_lines.len() - 1;
        }

        if already_found {
            Ok(Some((start_index, end_index)))
        } else {
            Ok(None)
        }
    }
    fn find_entry(&self, section_name: &str, entry_name: &str, section_start: usize, section_end: usize) -> Result<Option<usize>, ConfigError> {
        let compare_str = entry_name.to_uppercase();

        let mut already_found = false;
        let mut found_loc: usize = 0;

        let mut line_iter = section_start;
        while line_iter <= section_end {
            let mut first_word = String::new();
            for character in self.conf_file_lines[line_iter].chars() {
                if character.is_whitespace() || character == '=' {
                    break;
                }
                first_word.push(character);
            }
            if first_word.to_uppercase() == compare_str {
                if !already_found {
                    already_found = true;
                    found_loc = line_iter
                } else {
                    return Err(ConfigError::RedundantEntry(section_name.to_owned(), entry_name.to_owned(), found_loc, line_iter));
                }
            }

            line_iter += 1;
        }

        if already_found {
            Ok(Some(found_loc))
        } else {
            Ok(None)
        }
    }
    fn check_obsolete_entries(&self, config_file_path: &path::Path) -> Result<(), ConfigError> {

        for section_iter in 0..self.config_sections.len() {

            match self.find_section(&self.config_sections[section_iter].section_name)? {
                Some((start_index, end_index)) => {
                    for entry_iter in 0..self.config_sections[section_iter].obsolete_entries.len() {

                        match self.find_entry(&self.config_sections[section_iter].section_name, &self.config_sections[section_iter].obsolete_entries[entry_iter], start_index, end_index)? {
                            Some(loc) => {
                                warn!("The entry `{}' of the `[{}]' section at line {} of `{}' is from an older version of the emulator and is no longer used, it can be safely removed from the configuration file.", self.config_sections[section_iter].obsolete_entries[entry_iter], self.config_sections[section_iter].section_name, loc + 1, config_file_path.display());
                            },
                            None => { },
                        }
                    }
                },
                None => { }
            };
        }
        Ok(())
    }
    fn write_config_file(&self) -> Result<(), ConfigError> {

        // Use CR/LF on Windows, and plain LF everywhere else:
        let eol_mark = match cfg!(target_os = "windows") {
            true  => { "\r\n" },
            false => { "\n" },
        };

        let mut out_file = fs::File::create(&self.config_file_path)?;
        for line in &self.conf_file_lines {
            out_file.write_all(line.as_bytes())?;
            out_file.write_all(eol_mark.as_bytes())?;
        }

        Ok(())
    }
    fn parse_entry_specifier(entry_specifier: &str) -> Result<(String, String), ConfigError> {
        let mut section_acc = String::new();
        let mut entry_acc = String::new();

        let mut have_section = false;

        for character in entry_specifier.chars() {
            if !have_section {
                if character == '_' {
                    have_section = true;
                } else {
                    section_acc.push(character);
                }
            } else {
                entry_acc.push(character);
            }
        }
        let section = section_acc;
        let entry   = entry_acc;

        if !have_section {
            Err(ConfigError::EntrySpecNoEntryNameSpecified(entry_specifier.to_owned()))
        } else if section.is_empty() {
            Err(ConfigError::EntrySpecNoSectionNameSpecified(entry_specifier.to_owned()))
        } else if entry.is_empty() {
            Err(ConfigError::EntrySpecNoEntryNameSpecified(entry_specifier.to_owned()))
        } else {
            Ok((section.to_lowercase(), entry.to_lowercase()))
        }
    }
    pub fn get_config_entry_current_state(&self, entry_specifier: &str) -> Result<String, ConfigError> {
        let (requested_section, requested_entry) = ConfigSystem::parse_entry_specifier(entry_specifier)?;

        for section_iter in 0..self.config_sections.len() {
            let section_name_lc = self.config_sections[section_iter].section_name.to_lowercase();

            if section_name_lc == requested_section {
                let (start_index, end_index) = match self.find_section(&self.config_sections[section_iter].section_name)? {
                    Some((found_start_index, found_end_index)) => { (found_start_index, found_end_index) },
                    None => {
                        panic!("ConfigSystem::get_config_entry_current_state(): Section {} is missing in the configuration file text buffer, this is a bug", self.config_sections[section_iter].section_name);
                    }
                };
                for entry_iter in 0..self.config_sections[section_iter].entries.len() {
                    let entry_name_lc = self.config_sections[section_iter].entries[entry_iter].entry_name.to_lowercase();

                    if entry_name_lc == requested_entry {
                        let entry_loc = match self.find_entry(&self.config_sections[section_iter].section_name, &self.config_sections[section_iter].entries[entry_iter].entry_name, start_index, end_index)? {
                            Some(loc) => { loc },
                            None => {
                                panic!("ConfigSystem::get_config_entry_current_state(): Entry {} of Section {} is missing in the configuration file text buffer, this is a bug", self.config_sections[section_iter].entries[entry_iter].entry_name, self.config_sections[section_iter].section_name);
                            },
                        };
                        let assignee = retrieve_entry_assignee(&self.conf_file_lines[entry_loc]);
                        return Ok(format!("{}_{} = {}", section_name_lc, entry_name_lc, assignee));
                    }
                }
            }
        }
        Err(ConfigError::EntrySpecNoSuchConfigEntry(entry_specifier.to_owned()))
    }
    pub fn get_config_entry_current_state_all(&self) -> Result<Vec<String>, ConfigError> {
        let mut entry_state_collection = Vec::new();

        for section_iter in 0..self.config_sections.len() {
            let section_name_lc = self.config_sections[section_iter].section_name.to_lowercase();

            let (start_index, end_index) = match self.find_section(&self.config_sections[section_iter].section_name)? {
                Some((found_start_index, found_end_index)) => { (found_start_index, found_end_index) },
                None => {
                    panic!("ConfigSystem::get_config_entry_current_state_all(): Section {} is missing in the configuration file text buffer, this is a bug", self.config_sections[section_iter].section_name);
                }
            };
            for entry_iter in 0..self.config_sections[section_iter].entries.len() {
                let entry_name_lc = self.config_sections[section_iter].entries[entry_iter].entry_name.to_lowercase();

                let entry_loc = match self.find_entry(&self.config_sections[section_iter].section_name, &self.config_sections[section_iter].entries[entry_iter].entry_name, start_index, end_index)? {
                    Some(loc) => { loc },
                    None => {
                        panic!("ConfigSystem::get_config_entry_current_state_all(): Entry {} of Section {} is missing in the configuration file text buffer, this is a bug", self.config_sections[section_iter].entries[entry_iter].entry_name, self.config_sections[section_iter].section_name);
                    },
                };
                let assignee = retrieve_entry_assignee(&self.conf_file_lines[entry_loc]);

                entry_state_collection.push(format!("{}_{} = {}", section_name_lc, entry_name_lc, assignee));
            }
        }

        Ok(entry_state_collection)
    }
    pub fn change_config_entry(&mut self, entry_specifier: &str, invocation_text: &str) -> Result<ConfigChangeApplyAction, ConfigError> {
        let (requested_section, requested_entry) = ConfigSystem::parse_entry_specifier(entry_specifier)?;

        for section_iter in 0..self.config_sections.len() {
            let section_name_lc = self.config_sections[section_iter].section_name.to_lowercase();

            if section_name_lc == requested_section {
                let (start_index, end_index) = match self.find_section(&self.config_sections[section_iter].section_name)? {
                    Some((found_start_index, found_end_index)) => { (found_start_index, found_end_index) },
                    None => {
                        panic!("ConfigSystem::get_config_entry_current_state(): Section {} is missing in the configuration file text buffer, this is a bug", self.config_sections[section_iter].section_name);
                    }
                };
                for entry_iter in 0..self.config_sections[section_iter].entries.len() {
                    let entry_name_lc = self.config_sections[section_iter].entries[entry_iter].entry_name.to_lowercase();

                    if entry_name_lc == requested_entry {
                        let entry_loc = match self.find_entry(&self.config_sections[section_iter].section_name, &self.config_sections[section_iter].entries[entry_iter].entry_name, start_index, end_index)? {
                            Some(loc) => { loc },
                            None => {
                                panic!("ConfigSystem::get_config_entry_current_state(): Entry {} of Section {} is missing in the configuration file text buffer, this is a bug", self.config_sections[section_iter].entries[entry_iter].entry_name, self.config_sections[section_iter].section_name);
                            },
                        };
                        (self.config_sections[section_iter].entries[entry_iter].parse_entry)(ConfigInfoSource::from_external_source(&self.config_sections[section_iter].section_name, &self.config_sections[section_iter].entries[entry_iter].entry_name, invocation_text), &mut self.config_items)?;
                        match (self.config_sections[section_iter].entries[entry_iter].update_line)(ConfigInfoSource::from_config_file(entry_loc, &self.conf_file_lines[entry_loc]), &mut self.config_items) {
                            Some(updated_line) => {
                                self.conf_file_lines[entry_loc] = updated_line;
                                self.write_config_file()?;
                                return Ok(self.config_sections[section_iter].entries[entry_iter].apply_action);
                            },
                            None => {
                                return Ok(ConfigChangeApplyAction::AlreadyUpToDate);
                            },
                        }
                    }
                }
            }
        }
        Err(ConfigError::EntrySpecNoSuchConfigEntry(entry_specifier.to_owned()))
    }
}

// Find the %AppData% directory on Windows:
fn find_appdata() -> Option<path::PathBuf> {
    let mut search_result = None;

    for (varname_osstr, value_osstr) in env::vars_os() {
        match varname_osstr.into_string() {
            Ok(varname_str) => {
                if varname_str.to_uppercase() == "APPDATA" {
                    search_result = Some(value_osstr);
                    break;
                }
            },
            Err(..) => { },
        }
    }
    let search_result = search_result;

    match search_result {
        Some(path_osstr) => {
            Some((path_osstr.as_ref() as &path::Path).to_owned())
        }
        None => {
            None
        },
    }
}

// Get the default location of the project's configuration directory:
pub fn get_default_config_dir_path() -> path::PathBuf {

    if cfg!(target_os = "windows") {
        let mut config_dir_path = find_appdata().expect(".expect() call: Failed to find the %AppData% directory");
        config_dir_path.push(WINDOWS_DEV_NAME);
        config_dir_path.push(WINDOWS_PROJ_NAME);

        config_dir_path
    } else {
        let mut config_dir_path = home::home_dir().expect(".expect() call: Failed to find the home directory");
        config_dir_path.push(UNIX_HIDDEN_DIR_NAME);

        config_dir_path
    }
}

// Check whether the configuration directory exists, and if not, create it:
fn check_config_dir<P: AsRef<path::Path>>(config_dir_in: P) -> bool {

    let config_dir = config_dir_in.as_ref() as &path::Path;

    if !config_dir.exists() {
        match fs::create_dir_all(&config_dir) {
            Ok(()) => {
                info!("Created configuration directory `{}'.", config_dir.display());

                true
            },
            Err(error) => {
                error!("Failed to create configuration directory `{}': {}.", config_dir.display(), error);

                false
            },
        }
    } else if config_dir.exists() && !config_dir.is_dir() {
        error!("Failed to create configuration directory `{}': {}.", config_dir.display(), "A non-directory filesystem entry of the same name already exists, please rename/remove it and try again");
        false
    } else if config_dir.exists() && config_dir.is_dir() {
        info!("Found existing configuration directory `{}'.", config_dir.display());
        true
    } else {
        false
    }
}

// Load the configuration file into a vector of strings representing lines:
fn load_config_file<P: AsRef<path::Path>>(config_file_path_in: P)
                   -> Result<Vec<String>, ConfigError> {

    let config_file_path = config_file_path_in.as_ref() as &path::Path;
    if config_file_path.exists() {
        // Load everything:
        let mut config_file = fs::File::open(config_file_path)?;
        let mut buffer = String::new();
        config_file.read_to_string(&mut buffer)?;

        // Split it into lines:
        let mut current_line = String::new();
        let mut line_collection: Vec<String> = Vec::new();

        for current_char in buffer.chars() {
            // For line-splitting, it is assumed that either an LF or CR/LF is
            // the line separator.  The code ignores any CR, so both of the
            // above cases end up with LF as the line separator.

            if current_char == '\n' {
                line_collection.push(current_line.trim().to_owned());
                current_line = String::new();
            } else if current_char == '\r' {
                // Nothing.
            } else {
                current_line.push(current_char);
            }
        }
        if !current_line.is_empty() {
            line_collection.push(current_line.trim().to_owned());
        }

        info!("Successfully loaded the configuration file `{}'.", config_file_path.display());
        Ok(line_collection)
    } else {
        // Nothing to load:
        fs::File::create(config_file_path)?;
        info!("Created new configuration file `{}'.", config_file_path.display());
        Ok(Vec::new())
    }
}

// Configuration sections:
fn new_config_sections() -> Box<[ConfigSection]> {
    let mut sections: Vec<ConfigSection> = Vec::new();

    sections.push(new_general_section());
    sections.push(new_keyboard_section());
    sections.push(new_video_section());
    sections.push(new_cassette_section());

    sections.into_boxed_slice()
}

fn retrieve_entry_assignee(entry_string: &str) -> String {
    let mut chars = entry_string.chars();
    let mut current_char = chars.next();

    // Skip the name:
    while current_char != None {
        let current_char_ch = current_char.unwrap();
        if current_char_ch == '=' {
            break;
        } else {
            current_char = chars.next();
        }
    }
    current_char = chars.next();

    // Skip any white-spaces after the equals sign:
    while current_char != None {
        let current_char_ch = current_char.unwrap();
        if !current_char_ch.is_whitespace() {
            break;
        } else {
            current_char = chars.next();
        }
    }

    // Construct a string from the argument:
    let mut argument = String::new();
    while current_char != None {
        let current_char_ch = current_char.unwrap();

        argument.push(current_char_ch);
        current_char = chars.next();
    }

    argument
}

// Example of a valid resolution argument: `1024x768'.
fn parse_resolution_argument(entry_argument: &str) -> Option<(u32, u32)> {

    let mut have_width = false;
    let mut width = 0;

    let mut number_collector = String::new();

    for current_char in entry_argument.chars() {
        if current_char == 'x' || current_char == 'X' {
            if !have_width {
                width = match number_collector.parse::<u32>() {
                    Ok(result) => { result },
                    Err(..) => { return None; },
                };
                have_width = true;
                number_collector = String::new();
            } else {
                return None;
            }
        } else if current_char.is_digit(10) {
            number_collector.push(current_char);
        } else {
            return None;
        }
    }

    if !have_width {
        None
    } else {
        match number_collector.parse::<u32>() {
            Ok(height) => { Some((width, height)) },
            Err(..) => { None },
        }
    }
}

// Example of a valid color argument: `#00FF00'.
fn parse_color_argument(entry_argument: &str) -> Option<(u8, u8, u8)> {

    let mut chars = entry_argument.chars();

    match chars.next() {

        Some(first_character) => {
            if first_character == '#' {

                // Retrieve the digits.
                let rh_c = match chars.next() {
                    Some(character) => { character },
                    None => { return None }
                };
                let rl_c = match chars.next() {
                    Some(character) => { character },
                    None => { return None }
                };
                let gh_c = match chars.next() {
                    Some(character) => { character },
                    None => { return None }
                };
                let gl_c = match chars.next() {
                    Some(character) => { character },
                    None => { return None }
                };
                let bh_c = match chars.next() {
                    Some(character) => { character },
                    None => { return None }
                };
                let bl_c = match chars.next() {
                    Some(character) => { character },
                    None => { return None }
                };

                // There must not be any other character after these.
                match chars.next() {
                    Some(..) => { return None; },
                    None => { },
                }

                // Convert them to their numberic form:
                let rh = match rh_c.to_digit(16) {
                    Some(digit) => { digit as u8 }
                    None => { return None; }
                };
                let rl = match rl_c.to_digit(16) {
                    Some(digit) => { digit as u8 }
                    None => { return None; }
                };
                let gh = match gh_c.to_digit(16) {
                    Some(digit) => { digit as u8 }
                    None => { return None; }
                };
                let gl = match gl_c.to_digit(16) {
                    Some(digit) => { digit as u8 }
                    None => { return None; }
                };
                let bh = match bh_c.to_digit(16) {
                    Some(digit) => { digit as u8 }
                    None => { return None; }
                };
                let bl = match bl_c.to_digit(16) {
                    Some(digit) => { digit as u8 }
                    None => { return None; }
                };
                Some(((rh << 4) | rl,
                      (gh << 4) | gl,
                      (bh << 4) | bl))
            } else {
                None
            }
        },
        None => { None },
    }
}

// Examples of valid boolean arguments: `True', `false', `1', `0'.
fn parse_bool_argument(entry_argument: &str) -> Option<bool> {

    if entry_argument.len() == 1 {
        match entry_argument.chars().next() {
            Some(first_character) => {
                match first_character {
                    '1' => { Some(true)  },
                    '0' => { Some(false) },
                     _  => { None },
                }
            },
            None => { None },
        }
    } else {
        if entry_argument.to_uppercase() == "TRUE" {
            Some(true)
        } else if entry_argument.to_uppercase() == "FALSE" {
            Some(false)
        } else {
            None
        }
    }
}

// The general section and entries:
fn update_line_general_level_1_rom(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.general_level_1_rom.clone();

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_general_level_1_rom(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.general_level_1_rom != new_val {
        config_items.general_level_1_rom = new_val.clone();
        match new_val {
            Some(value) => {
                Some(format!("level_1_rom = {}", value))
            },
            None => {
                Some("level_1_rom = none".to_owned())
            },
        }
    } else {
        None
    }
}
fn update_line_general_level_2_rom(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.general_level_2_rom.clone();

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_general_level_2_rom(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.general_level_2_rom != new_val {
        config_items.general_level_2_rom = new_val.clone();
        match new_val {
            Some(value) => {
                Some(format!("level_2_rom = {}", value))
            },
            None => {
                Some("level_2_rom = none".to_owned())
            },
        }
    } else {
        None
    }
}
fn update_line_general_misc_rom(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.general_misc_rom.clone();

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_general_misc_rom(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.general_misc_rom != new_val {
        config_items.general_misc_rom = new_val.clone();
        match new_val {
            Some(value) => {
                Some(format!("misc_rom = {}", value))
            },
            None => {
                Some("misc_rom = none".to_owned())
            },
        }
    } else {
        None
    }
}

fn parse_entry_general_level_1_rom(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    let argument = info_source.argument_text();

    if argument.to_uppercase() == "NONE" {
        config_items.general_level_1_rom = None;
    } else {
        config_items.general_level_1_rom = Some(argument);
    }

    Ok(())
}
fn parse_entry_general_level_2_rom(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    let argument = info_source.argument_text();

    if argument.to_uppercase() == "NONE" {
        config_items.general_level_2_rom = None;
    } else {
        config_items.general_level_2_rom = Some(argument);
    }

    Ok(())
}
fn parse_entry_general_misc_rom(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    let argument = info_source.argument_text();

    if argument.to_uppercase() == "NONE" {
        config_items.general_misc_rom = None;
    } else {
        config_items.general_misc_rom = Some(argument);
    }

    Ok(())
}

fn update_line_general_default_rom(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.general_default_rom;

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_general_default_rom(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.general_default_rom != new_val {
        config_items.general_default_rom = new_val;
        Some(format!("default_rom = {}", new_val))
    } else {
        None
    }
}
fn parse_entry_general_default_rom(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    let argument = match info_source.argument_text().parse::<u32>() {
        Ok(result) => { result },
        Err(error) => { return Err(ConfigError::EntryIntParsingError(info_source, error)); },
    };

    if argument >= 1 && argument <= 3 {
        config_items.general_default_rom = argument;
        Ok(())
    } else {
        Err(ConfigError::DefaultRomOutOfRange(info_source, argument))
    }
}

fn update_line_general_ram_size(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.general_ram_size;

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_general_ram_size(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.general_ram_size != new_val {
        config_items.general_ram_size = new_val;
        if (new_val % 1024) == 0 {
            Some(format!("ram_size = {}K", new_val / 1024))
        } else {
            Some(format!("ram_size = {}", new_val))
        }
    } else {
        None
    }
}
fn parse_entry_general_ram_size(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    let mut new_ram_size = 0;
    let mut found_suffix = false;

    for character in info_source.argument_text().chars() {
        if !found_suffix {
            match character.to_digit(10) {
                Some(digit) => {
                    new_ram_size = (new_ram_size * 10) + digit;
                },
                None => {
                    if character == 'k' || character == 'K' {
                        new_ram_size *= 1024;
                        found_suffix = true;
                    } else {
                        return Err(ConfigError::InvalidRamSpecifier(info_source));
                    }
                },
            }
        } else {
            return Err(ConfigError::InvalidRamSpecifier(info_source));
        }
    }
    if new_ram_size <= 48 * 1024 {
        config_items.general_ram_size = new_ram_size;
        Ok(())
    } else {
        Err(ConfigError::TooMuchRamRequested(info_source, new_ram_size))
    }
}

fn new_handler_general_level_1_rom() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("".to_owned());
    default_text.push("; The name of the system ROM image files (name, path, or the keyword `none').".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; You only need to provide one of these rom files.  The three fields exist".to_owned());
    default_text.push("; solely for the sake of convenience, when you want to have different roms".to_owned());
    default_text.push("; and to be able to switch between them without having to keep changing".to_owned());
    default_text.push("; the names here in the configuration file.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; If you specify a name, the program will look for the rom file in the".to_owned());
    default_text.push("; configuration directory, which is where this file resides.  If you want".to_owned());
    default_text.push("; to store the rom(s) in a different directory, specify a full path to the".to_owned());
    default_text.push("; rom files.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("level_1_rom = none".to_owned());

    ConfigEntry {
        entry_name:   "level_1_rom".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::RomChange(1),
        update_line:  update_line_general_level_1_rom,
        parse_entry:  parse_entry_general_level_1_rom,
    }
}
fn new_handler_general_level_2_rom() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("level_2_rom = none".to_owned());

    ConfigEntry {
        entry_name:   "level_2_rom".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::RomChange(2),
        update_line:  update_line_general_level_2_rom,
        parse_entry:  parse_entry_general_level_2_rom,
    }
}
fn new_handler_general_misc_rom() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("misc_rom = none".to_owned());
    default_text.push("".to_owned());

    ConfigEntry {
        entry_name:   "misc_rom".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::RomChange(3),
        update_line:  update_line_general_misc_rom,
        parse_entry:  parse_entry_general_misc_rom,
    }
}
fn new_handler_general_default_rom() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("".to_owned());
    default_text.push("; Which of the three rom files to use (1 to 3); this can be overridden on".to_owned());
    default_text.push("; program startup, using the -1, -2 and -3 command-line arguments.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("default_rom = 2".to_owned());
    default_text.push("".to_owned());

    ConfigEntry {
        entry_name:   "default_rom".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::UpdateDefaultRomSelection,
        update_line:  update_line_general_default_rom,
        parse_entry:  parse_entry_general_default_rom,
    }
}
fn new_handler_general_ram_size() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("".to_owned());
    default_text.push("; The amount of memory the machine has installed.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; Common values are 4K, 16K, 32K and 48K.  You can have at most 48K installed.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; You can specify the amount of memory either in bytes (without a suffix), or".to_owned());
    default_text.push("; in kilobytes, by appending the K suffix to the number.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("ram_size = 16K".to_owned());
    default_text.push("".to_owned());

    ConfigEntry {
        entry_name:   "ram_size".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::ChangeRamSize,
        update_line:  update_line_general_ram_size,
        parse_entry:  parse_entry_general_ram_size,
    }
}

fn new_general_section() -> ConfigSection {
    let mut entries: Vec<ConfigEntry> = Vec::new();

    entries.push(new_handler_general_level_1_rom());
    entries.push(new_handler_general_level_2_rom());
    entries.push(new_handler_general_misc_rom());
    entries.push(new_handler_general_default_rom());
    entries.push(new_handler_general_ram_size());

    let obsolete_entries: Vec<String> = Vec::new();

    ConfigSection {
        section_name:     "General".to_owned(),
        entries:          entries.into_boxed_slice(),
        obsolete_entries: obsolete_entries.into_boxed_slice(),
    }
}

// The keyboard section and entries:
fn update_line_keyboard_ms_per_keypress(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.keyboard_ms_per_keypress;

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_keyboard_ms_per_keypress(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.keyboard_ms_per_keypress != new_val {
        config_items.keyboard_ms_per_keypress = new_val;
        Some(format!("ms_per_keypress = {}", new_val))
    } else {
        None
    }
}

fn parse_entry_keyboard_ms_per_keypress(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    let argument = match info_source.argument_text().parse::<u32>() {
        Ok(result) => { result },
        Err(error) => { return Err(ConfigError::EntryIntParsingError(info_source, error)); },
    };

    config_items.keyboard_ms_per_keypress = argument;
    Ok(())
}

fn new_handler_keyboard_ms_per_keypress() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();
    default_text.push("".to_owned());
    default_text.push("; The minimum time it takes to press down or release a key, in miliseconds.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; The purpose of this is to make sure that the input routine can catch the".to_owned());
    default_text.push("; keyboard updates, since there's no dedicated circuitry for this in the".to_owned());
    default_text.push("; machine, just the CPU probing the keyboard matrix.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; A value between 5 to 50 is recommended.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("ms_per_keypress = 20".to_owned());
    default_text.push("".to_owned());

    ConfigEntry {
        entry_name:   "ms_per_keypress".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::UpdateMsPerKeypress,
        update_line:  update_line_keyboard_ms_per_keypress,
        parse_entry:  parse_entry_keyboard_ms_per_keypress,
    }
}

fn new_keyboard_section() -> ConfigSection {
    let mut entries: Vec<ConfigEntry> = Vec::new();
    entries.push(new_handler_keyboard_ms_per_keypress());

    let obsolete_entries: Vec<String> = Vec::new();

    ConfigSection {
        section_name:     "Keyboard".to_owned(),
        entries:          entries.into_boxed_slice(),
        obsolete_entries: obsolete_entries.into_boxed_slice(),
    }
}

// The video section and entries:
fn update_line_video_windowed_resolution(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.video_windowed_resolution;

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_video_windowed_resolution(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.video_windowed_resolution != new_val {
        config_items.video_windowed_resolution = new_val;
        let (width, height) = new_val;
        Some(format!("windowed_resolution = {}x{}", width, height))
    } else {
        None
    }
}
fn update_line_video_fullscreen_resolution(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.video_fullscreen_resolution;

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_video_fullscreen_resolution(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.video_fullscreen_resolution != new_val {
        config_items.video_fullscreen_resolution = new_val;
        let (width, height) = new_val;
        Some(format!("fullscreen_resolution = {}x{}", width, height))
    } else {
        None
    }
}
fn parse_entry_video_windowed_resolution(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    match parse_resolution_argument(info_source.argument_text().as_str()) {
        Some(resolution) => {
            config_items.video_windowed_resolution = resolution;
            Ok(())
        },
        None => {
            Err(ConfigError::InvalidResolutionSpecifier(info_source))
        }
    }
}
fn parse_entry_video_fullscreen_resolution(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    match parse_resolution_argument(info_source.argument_text().as_str()) {
        Some(resolution) => {
            config_items.video_fullscreen_resolution = resolution;
            Ok(())
        },
        None => {
            Err(ConfigError::InvalidResolutionSpecifier(info_source))
        }
    }
}

fn update_line_video_bg_color(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.video_bg_color;

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_video_bg_color(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.video_bg_color != new_val {
        config_items.video_bg_color = new_val;
        let (red, green, blue) = new_val;
        Some(format!("bg_color = #{:02X}{:02X}{:02X}", red, green, blue))
    } else {
        None
    }
}
fn update_line_video_fg_color(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.video_fg_color;

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_video_fg_color(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.video_fg_color != new_val {
        config_items.video_fg_color = new_val;
        let (red, green, blue) = new_val;
        Some(format!("fg_color = #{:02X}{:02X}{:02X}", red, green, blue))
    } else {
        None
    }
}
fn parse_entry_video_bg_color(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    match parse_color_argument(info_source.argument_text().as_str()) {
        Some(color) => {
            config_items.video_bg_color = color;
            Ok(())
        },
        None => {
            Err(ConfigError::InvalidColorSpecifier(info_source))
        }
    }
}
fn parse_entry_video_fg_color(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    match parse_color_argument(info_source.argument_text().as_str()) {
        Some(color) => {
            config_items.video_fg_color = color;
            Ok(())
        },
        None => {
            Err(ConfigError::InvalidColorSpecifier(info_source))
        }
    }
}

fn update_line_video_desktop_fullscreen_mode(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.video_desktop_fullscreen_mode;

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_video_desktop_fullscreen_mode(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.video_desktop_fullscreen_mode != new_val {
        config_items.video_desktop_fullscreen_mode = new_val;
        Some(format!("desktop_fullscreen_mode = {}", if new_val { "true" } else { "false" }))
    } else {
        None
    }
}
fn parse_entry_video_desktop_fullscreen_mode(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    match parse_bool_argument(info_source.argument_text().as_str()) {
        Some(value) => {
            config_items.video_desktop_fullscreen_mode = value;
            Ok(())
        },
        None => {
            Err(ConfigError::InvalidBoolSpecifier(info_source))
        }
    }
}

fn update_line_video_use_hw_accel(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.video_use_hw_accel;

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_video_use_hw_accel(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.video_use_hw_accel != new_val {
        config_items.video_use_hw_accel = new_val;
        Some(format!("use_hw_accel = {}", if new_val { "true" } else { "false" }))
    } else {
        None
    }
}
fn parse_entry_video_use_hw_accel(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    match parse_bool_argument(info_source.argument_text().as_str()) {
        Some(value) => {
            config_items.video_use_hw_accel = value;
            Ok(())
        },
        None => {
            Err(ConfigError::InvalidBoolSpecifier(info_source))
        }
    }
}

fn update_line_video_use_vsync(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.video_use_vsync;

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_video_use_vsync(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.video_use_vsync != new_val {
        config_items.video_use_vsync = new_val;
        Some(format!("use_vsync = {}", if new_val { "true" } else { "false" }))
    } else {
        None
    }
}
fn parse_entry_video_use_vsync(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    match parse_bool_argument(info_source.argument_text().as_str()) {
        Some(value) => {
            config_items.video_use_vsync = value;
            Ok(())
        },
        None => {
            Err(ConfigError::InvalidBoolSpecifier(info_source))
        }
    }
}

fn update_line_video_character_generator(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.video_character_generator;

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_video_character_generator(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.video_character_generator != new_val {
        config_items.video_character_generator = new_val;
        Some(format!("character_generator = {}", new_val))
    } else {
        None
    }
}
fn parse_entry_video_character_generator(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    let argument = match info_source.argument_text().parse::<u32>() {
        Ok(result) => { result },
        Err(error) => { return Err(ConfigError::EntryIntParsingError(info_source, error)); },
    };

    if argument >= 1 && argument <= 3 {
        config_items.video_character_generator = argument;
        Ok(())
    } else {
        Err(ConfigError::CharacterGeneratorOutOfRange(info_source, argument))
    }
}

fn update_line_video_lowercase_mod(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.video_lowercase_mod;

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_video_lowercase_mod(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.video_lowercase_mod != new_val {
        config_items.video_lowercase_mod = new_val;
        Some(format!("lowercase_mod = {}", if new_val { "true" } else { "false" }))
    } else {
        None
    }
}
fn parse_entry_video_lowercase_mod(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    match parse_bool_argument(info_source.argument_text().as_str()) {
        Some(value) => {
            config_items.video_lowercase_mod = value;
            Ok(())
        },
        None => {
            Err(ConfigError::InvalidBoolSpecifier(info_source))
        }
    }
}


fn new_handler_video_windowed_resolution() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("".to_owned());
    default_text.push("; The screen resolution, as WIDTHxHEIGHT, in windowed and full-screen mode.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; The native resolution of the emulator is 512x384 (4:3 aspect ratio),".to_owned());
    default_text.push("; recommended are multiples of this resolution, like 1024x768.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; I'd advise against 648x480, as it looks quite crummy because of the scaling.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; The fullscreen resolution is only taken into account if the true fullscreen".to_owned());
    default_text.push("; mode is selected.  In the desktop fullscreen mode, the emulator adapts to".to_owned());
    default_text.push("; your current screen resolution.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("windowed_resolution = 512x384".to_owned());

    ConfigEntry {
        entry_name:   "windowed_resolution".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::ChangeWindowedResolution,
        update_line:  update_line_video_windowed_resolution,
        parse_entry:  parse_entry_video_windowed_resolution,
    }
}
fn new_handler_video_fullscreen_resolution() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();
    default_text.push("fullscreen_resolution = 1024x768".to_owned());
    default_text.push("".to_owned());

    ConfigEntry {
        entry_name:   "fullscreen_resolution".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::ChangeFullscreenResolution,
        update_line:  update_line_video_fullscreen_resolution,
        parse_entry:  parse_entry_video_fullscreen_resolution,
    }
}
fn new_handler_video_bg_color() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("".to_owned());
    default_text.push("; The colors to use for the screen background and foreground, specified using".to_owned());
    default_text.push("; the hex (#RRGGBB) format.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; By default, the background is black, #000000, and the foreground is green,".to_owned());
    default_text.push("; #00FF00; other common choices for the foreground are amber, #FFBF00, and".to_owned());
    default_text.push("; gray, #A8A8A8.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("bg_color = #000000".to_owned());

    ConfigEntry {
        entry_name:   "bg_color".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::ChangeColor,
        update_line:  update_line_video_bg_color,
        parse_entry:  parse_entry_video_bg_color,
    }
}
fn new_handler_video_fg_color() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("fg_color = #00FF00".to_owned());
    default_text.push("".to_owned());

    ConfigEntry {
        entry_name:   "fg_color".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::ChangeColor,
        update_line:  update_line_video_fg_color,
        parse_entry:  parse_entry_video_fg_color,
    }
}
fn new_handler_video_desktop_fullscreen_mode() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("".to_owned());
    default_text.push("; Use the desktop fullscreen mode (true or false).".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; If set to true, the emulator doesn't change the resolution of your screen".to_owned());
    default_text.push("; when going into full-screen mode, and instead acts as a borderless window".to_owned());
    default_text.push("; that takes up the whole screen.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("desktop_fullscreen_mode = false".to_owned());
    default_text.push("".to_owned());

    ConfigEntry {
        entry_name:   "desktop_fullscreen_mode".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::ChangeFullscreenResolution,
        update_line:  update_line_video_desktop_fullscreen_mode,
        parse_entry:  parse_entry_video_desktop_fullscreen_mode,
    }
}
fn new_handler_video_use_hw_accel() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("".to_owned());
    default_text.push("; Use hardware video acceleration (true or false).".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; With video acceleration enabled, the emulator will use your graphics card".to_owned());
    default_text.push("; to render the screen directly.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; This is mainly useful when not using the emulator's native resolution, as it".to_owned());
    default_text.push("; allows the GPU to stretch the image, instead of having the CPU stretch it.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("use_hw_accel = false".to_owned());
    default_text.push("".to_owned());

    ConfigEntry {
        entry_name:   "use_hw_accel".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::ChangeHwAccelUsage,
        update_line:  update_line_video_use_hw_accel,
        parse_entry:  parse_entry_video_use_hw_accel,
    }
}
fn new_handler_video_use_vsync() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("".to_owned());
    default_text.push("; Use vertical synchronization (true or false).".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; Vith vsync enabled, the screen contents are updated in sync with the screen's".to_owned());
    default_text.push("; refresh rate.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("use_vsync = false".to_owned());
    default_text.push("".to_owned());

    ConfigEntry {
        entry_name:   "use_vsync".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::ChangeVsyncUsage,
        update_line:  update_line_video_use_vsync,
        parse_entry:  parse_entry_video_use_vsync,
    }
}
fn new_handler_video_character_generator() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("".to_owned());
    default_text.push("; Character generator to use (1 to 3).".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; There are three variants of the character generator commonly found in".to_owned());
    default_text.push("; a TRS-80 Model I, available for you to choose:".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";     1 - A very old version of the Model I font, found in only a few machines,".to_owned());
    default_text.push(";         that has standard ASCII [ \\ ] ^ instead of directional arrows.".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";         Level II basic puts odd symbols from positions 0-31 onto the screen".to_owned());
    default_text.push(";         if you enable the lowercase mod.".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";     2 - This is the standard Model I character generator found in machines".to_owned());
    default_text.push(";         without the Radio Shack lowercase modification, including the".to_owned());
    default_text.push(";         arrows.".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";         Just like with the previous character generator, Level II basic".to_owned());
    default_text.push(";         puts odd symbols onto the screen if you enable the lowercase mod.".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";     3 - This is the replacement character generator you got with the".to_owned());
    default_text.push(";         Radio Shack lowercase mod.".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";         Positions 0-31 are a copy of the uppercase letters, to work around".to_owned());
    default_text.push(";         a bug in the Level II ROM.".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";         All characters without descenders are moved up one row.".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";".to_owned());
    default_text.push("character_generator = 2".to_owned());
    default_text.push("".to_owned());

    ConfigEntry {
        entry_name:   "character_generator".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::ChangeCharacterGenerator,
        update_line:  update_line_video_character_generator,
        parse_entry:  parse_entry_video_character_generator,
    }
}
fn new_handler_video_lowercase_mod() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("".to_owned());
    default_text.push("; Use the lowercase mod (true or false).".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; The original TRS-80 Model I machines lacked the ability to display lowercase".to_owned());
    default_text.push("; characters, but this could be remedied by a modification.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; It is advised to use character generator 3 with this modification enabled,".to_owned());
    default_text.push("; as without it, Level II basic puts odd symbols onto the screen instead of".to_owned());
    default_text.push("; the regular uppercase letters.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("lowercase_mod = false".to_owned());
    default_text.push("".to_owned());

    ConfigEntry {
        entry_name:   "lowercase_mod".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::ChangeLowercaseModUsage,
        update_line:  update_line_video_lowercase_mod,
        parse_entry:  parse_entry_video_lowercase_mod,
    }
}

fn new_video_section() -> ConfigSection {
    let mut entries: Vec<ConfigEntry> = Vec::new();

    entries.push(new_handler_video_windowed_resolution());
    entries.push(new_handler_video_fullscreen_resolution());
    entries.push(new_handler_video_bg_color());
    entries.push(new_handler_video_fg_color());
    entries.push(new_handler_video_desktop_fullscreen_mode());
    entries.push(new_handler_video_use_hw_accel());
    entries.push(new_handler_video_use_vsync());
    entries.push(new_handler_video_character_generator());
    entries.push(new_handler_video_lowercase_mod());

    let obsolete_entries: Vec<String> = Vec::new();

    ConfigSection {
        section_name:     "Video".to_owned(),
        entries:          entries.into_boxed_slice(),
        obsolete_entries: obsolete_entries.into_boxed_slice(),
    }
}

// The cassette section and entries:
fn update_line_cassette_file(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.cassette_file.clone();

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_cassette_file(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.cassette_file != new_val {
        config_items.cassette_file = new_val.clone();
        match new_val {
            Some(value) => {
                Some(format!("file = {}", value))
            },
            None => {
                Some("file = none".to_owned())
            },
        }
    } else {
        None
    }
}
fn update_line_cassette_file_format(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.cassette_file_format;

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_cassette_file_format(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.cassette_file_format != new_val {
        config_items.cassette_file_format = new_val;
        match new_val {
            cassette::Format::CAS => {
                Some("file_format = CAS".to_owned())
            },
            cassette::Format::CPT => {
                Some("file_format = CPT".to_owned())
            },
        }
    } else {
        None
    }
}
fn update_line_cassette_file_offset(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Option<String> {
    let new_val = config_items.cassette_file_offset;

    // Re-parse the entry, to see if it really changed and to see whether
    // an update really is neccessary.  On failure assume yes.
    let failed_read = match parse_entry_cassette_file_offset(info_source, config_items) {
        Ok(..)  => { false },
        Err(..) => { true  },
    };

    // Update only if we really need to update:
    if failed_read || config_items.cassette_file_offset != new_val {
        config_items.cassette_file_offset = new_val;
        Some(format!("file_offset = {}", new_val))
    } else {
        None
    }
}
fn parse_entry_cassette_file(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    let argument = info_source.argument_text();

    if argument.to_uppercase() == "NONE" {
        config_items.cassette_file = None;
    } else {
        config_items.cassette_file = Some(argument);
    }

    Ok(())
}
fn parse_entry_cassette_file_format(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    let argument = info_source.argument_text();
    let compare_str = argument.to_uppercase();

    if compare_str == "CAS" {
        config_items.cassette_file_format = cassette::Format::CAS;
        Ok(())
    } else if compare_str == "CPT" {
        config_items.cassette_file_format = cassette::Format::CPT;
        Ok(())
    } else {
        Err(ConfigError::InvalidCassetteFormatSpecifier(info_source))
    }
}
fn parse_entry_cassette_file_offset(info_source: ConfigInfoSource, config_items: &mut ConfigItems) -> Result<(), ConfigError> {
    let argument = match info_source.argument_text().parse::<usize>() {
        Ok(result) => { result },
        Err(error) => { return Err(ConfigError::EntryIntParsingError(info_source, error)); },
    };

    config_items.cassette_file_offset = argument;
    Ok(())
}
fn new_handler_cassette_file() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("".to_owned());
    default_text.push("; The name of the cassette file currently in the tape drive.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; You can either specify a full path to a cassette file, a simple file name".to_owned());
    default_text.push("; if you want the file to be located in the configuration directory, or the".to_owned());
    default_text.push("; keyword `none' to leave the tape drive empty.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; If the specified file doesn't exist yet, it will be created.".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";".to_owned());
    default_text.push("file = none".to_owned());
    default_text.push("".to_owned());

    ConfigEntry {
        entry_name:   "file".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::UpdateCassetteFile,
        update_line:  update_line_cassette_file,
        parse_entry:  parse_entry_cassette_file,
    }
}
fn new_handler_cassette_file_format() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("".to_owned());
    default_text.push("; Cassette file format selection (CAS or CPT):".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; Currently, the emulator supports two cassette file formats:".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";     CAS - A file containing the recovered bytes from the cassette.".to_owned());
    default_text.push(";           It is a fairly compact format, and it's compatible with other".to_owned());
    default_text.push(";           TRS-80 emulators that have cassette support.".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";     CPT - Cassette Pulse Train - A file containing exact values and timing".to_owned());
    default_text.push(";           (to the nearest microsecond) of the signals the TRS-80 cassette".to_owned());
    default_text.push(";           routine sends to the cassette output port to be recorded on the".to_owned());
    default_text.push(";           tape.".to_owned());
    default_text.push(";".to_owned());
    default_text.push(";           This format, originating from Tim Mann's xtrs emulator, emulates".to_owned());
    default_text.push(";           a perfect, noise-free cassette, so any cassette routines that even".to_owned());
    default_text.push(";           halfway worked on real hardware should work with it.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("file_format = CAS".to_owned());
    default_text.push("".to_owned());

    ConfigEntry {
        entry_name:   "file_format".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::UpdateCassetteFileFormat,
        update_line:  update_line_cassette_file_format,
        parse_entry:  parse_entry_cassette_file_format,
    }
}
fn new_handler_cassette_file_offset() -> ConfigEntry {
    let mut default_text: Vec<String> = Vec::new();

    default_text.push("".to_owned());
    default_text.push("; Current byte offset into the cassette.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; This value indicates how far the cassette is currently wound past the".to_owned());
    default_text.push("; beginning, in bytes.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; The cassette can be rewound to arbitrary locations, and thus several different".to_owned());
    default_text.push("; records/files can be recorded and later loaded from a single cassette, as long".to_owned());
    default_text.push("; as you keep track of where the different records/files are located, or request".to_owned());
    default_text.push("; files based on their filename (Level II BASIC).  See `/help cassette' in the".to_owned());
    default_text.push("; curses-based user interface for more details.".to_owned());
    default_text.push(";".to_owned());
    default_text.push("; Please note that if a format like CPT is used, setting the offset to arbitrary".to_owned());
    default_text.push("; locations might cause the data to be incorrectly parsed (since it may easily".to_owned());
    default_text.push("; get out of alignment), it is therefore advised to only explicitly set this".to_owned());
    default_text.push("; parameters to known-good values (ie. the beginning and end locations of the".to_owned());
    default_text.push("; individual records/files on the tape).".to_owned());
    default_text.push(";".to_owned());
    default_text.push("file_offset = 0".to_owned());
    default_text.push("".to_owned());

    ConfigEntry {
        entry_name:   "file_offset".to_owned(),
        default_text: default_text.into_boxed_slice(),
        apply_action: ConfigChangeApplyAction::UpdateCassetteFileOffset,
        update_line:  update_line_cassette_file_offset,
        parse_entry:  parse_entry_cassette_file_offset,
    }
}
fn new_cassette_section() -> ConfigSection {
    let mut entries: Vec<ConfigEntry> = Vec::new();

    entries.push(new_handler_cassette_file());
    entries.push(new_handler_cassette_file_format());
    entries.push(new_handler_cassette_file_offset());

    let mut obsolete_entries: Vec<String> = Vec::new();

    obsolete_entries.push("input_cassette".to_owned());
    obsolete_entries.push("output_cassette".to_owned());
    obsolete_entries.push("input_cassette_format".to_owned());
    obsolete_entries.push("output_cassette_format".to_owned());

    ConfigSection {
        section_name:    "Cassette".to_owned(),
        entries:          entries.into_boxed_slice(),
        obsolete_entries: obsolete_entries.into_boxed_slice(),
    }
}
