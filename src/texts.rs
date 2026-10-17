//! 畫面上的字：英文、繁體中文各一份；換語言 = 換一組 `Texts`

/// 支援的語言
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// 英文
    En,
    /// 繁體中文（台灣）
    ZhTw,
}

impl Lang {
    /// 從語言代碼判斷語言，不分大小寫，`_` 和 `-` 都可以；不支援的回傳 `None`
    ///
    /// 例：`en`、`en-US` → 英文；`zh-TW`、`zh_TW`、`zh-Hant`、`zh-Hant-TW`、`zh-HK` → 繁體中文
    pub fn from_code(code: &str) -> Option<Self> {
        let code = code.to_lowercase().replace('_', "-");
        if code == "en" || code.starts_with("en-") {
            Some(Self::En)
        } else if ["zh-tw", "zh-hant", "zh-hk", "zh-mo"]
            .iter()
            .any(|prefix| code.starts_with(prefix))
        {
            Some(Self::ZhTw)
        } else {
            None
        }
    }

    /// 系統的語言；讀不到或不支援就用英文
    pub fn from_system() -> Self {
        sys_locale::get_locale()
            .and_then(|code| Self::from_code(&code))
            .unwrap_or(Self::En)
    }

    /// 這個語言的那組字
    pub fn texts(self) -> &'static Texts {
        match self {
            Self::En => &EN,
            Self::ZhTw => &ZH_TW,
        }
    }
}

/// 一種語言的所有字
#[derive(Debug)]
pub struct Texts {
    /// 最上面一列的標題
    pub title: &'static str,
    /// 最下面一列的操作說明
    pub hints: &'static str,
}

const EN: Texts = Texts {
    title: " roh-cal",
    hints: " q quit",
};

const ZH_TW: Texts = Texts {
    title: " roh-cal",
    hints: " q 離開",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_code_accepts_common_spellings() {
        for code in ["en", "en-US", "EN_gb"] {
            assert_eq!(Lang::from_code(code), Some(Lang::En), "{code}");
        }
        for code in ["zh-TW", "zh_TW", "zh-Hant", "zh-Hant-TW", "zh-HK"] {
            assert_eq!(Lang::from_code(code), Some(Lang::ZhTw), "{code}");
        }
    }

    #[test]
    fn from_code_rejects_unsupported() {
        // 簡體中文、日文、空字串、像 en 但不是 en 的
        for code in ["zh-CN", "zh-Hans", "ja-JP", "", "eng"] {
            assert_eq!(Lang::from_code(code), None, "{code}");
        }
    }
}
