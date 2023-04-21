pub mod chat;
pub mod home;
pub mod image;
pub mod login;
pub mod prompt;
pub mod users;

pub use self::{chat::*, home::*, image::*, login::*, prompt::*, users::*};

use crate::components::navbar::NavElement;

use leptos::*;
use leptos_router::{use_navigate, NavigationError};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Page {
    #[default]
    Home,
    Users,
    UserAdd,
    UserEdit,
    UserPasswordChange,
    UserProfile,
    Chat,
    ChatView,
    Prompt,
    PromptGenerate,
    PromptList,
    PromptView,
    GenerateImage,
    GeneratedImageView,
    Login,
}

impl Page {
    pub fn root_page(&self) -> Self {
        match self {
            Self::Users
            | Self::UserAdd
            | Self::UserEdit
            | Self::UserPasswordChange
            | Self::UserProfile => Self::Users,
            Self::Chat | Self::ChatView => Self::Chat,
            Self::GenerateImage | Self::GeneratedImageView => Self::GenerateImage,
            Self::Prompt | Self::PromptGenerate | Self::PromptList | Self::PromptView => {
                Self::Prompt
            }
            Self::Home | Self::Login => *self,
        }
    }
    pub fn path(&self) -> &'static str {
        match self {
            Self::Home => "/",
            Self::Users => "/users",
            Self::UserAdd => "/users/add",
            Self::UserEdit => "/users/:username/edit",
            Self::UserPasswordChange => "/users/:username/password",
            Self::UserProfile => "/users/profile",
            Self::Chat => "/chat",
            Self::ChatView => "/chat/:chat_id",
            Self::Prompt | Self::PromptGenerate => "/prompt",
            Self::PromptList => "/prompt/history",
            Self::PromptView => "/prompt/:prompt_id",
            Self::GenerateImage => "/gen/image",
            Self::GeneratedImageView => "/gen/image/:image_id",
            Self::Login => "/login",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Home => "/icons/home.svg",
            Self::Users
            | Self::UserAdd
            | Self::UserEdit
            | Self::UserPasswordChange
            | Self::UserProfile => "/icons/users.svg",
            Self::Chat | Self::ChatView => "/icons/message-circle.svg",
            Self::Prompt | Self::PromptGenerate | Self::PromptList | Self::PromptView => {
                "/icons/terminal.svg"
            }
            Self::Login => "/icons/login.svg",
            Self::GenerateImage | Self::GeneratedImageView => "/icons/image.svg",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Users
            | Self::UserAdd
            | Self::UserEdit
            | Self::UserPasswordChange
            | Self::UserProfile => "Users",
            Self::Chat | Self::ChatView => "Chat",
            Self::Prompt => "Prompt",
            Self::PromptGenerate => "Generate Prompt",
            Self::PromptList => "Prompt History",
            Self::PromptView => "Prompt",
            Self::Login => "Login",
            Self::GenerateImage | Self::GeneratedImageView => "Generate Image",
        }
    }

    pub fn nav_display(&self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Users
            | Self::UserAdd
            | Self::UserEdit
            | Self::UserPasswordChange
            | Self::UserProfile => "Users",
            Self::Chat | Self::ChatView => "Chat",
            Self::Prompt | Self::PromptView => "Prompt",
            Self::PromptGenerate => "generate",
            Self::PromptList => "history",
            Self::Login => "Login",
            Self::GenerateImage | Self::GeneratedImageView => "Generate Image",
        }
    }

    pub fn main_user_pages() -> &'static [NavElement] {
        &[
            NavElement::Main(Self::Home),
            NavElement::Main(Self::Chat),
            NavElement::Sub(Self::Prompt, &[Self::PromptGenerate, Self::PromptList]),
            NavElement::Main(Self::GenerateImage),
        ]
    }

    pub fn main_admin_pages() -> &'static [NavElement] {
        &[
            NavElement::Main(Self::Home),
            NavElement::Main(Self::Users),
            NavElement::Main(Self::Chat),
            NavElement::Sub(Self::Prompt, &[Self::PromptGenerate, Self::PromptList]),
            NavElement::Main(Self::GenerateImage),
        ]
    }
}

impl AsRef<str> for Page {
    fn as_ref(&self) -> &str {
        self.path()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PageStack {
    stack: [Page; 2],
}
impl PageStack {
    pub fn new(stack: [Page; 2]) -> Self {
        Self { stack }
    }

    pub fn push(&mut self, page: Page) {
        self.stack[0] = self.stack[1];
        self.stack[1] = page;
    }

    pub fn current(&self) -> Page {
        self.stack[1]
    }

    pub fn parent(&self) -> Page {
        self.stack[0]
    }
}

pub fn goto_login_if_expired(
    cx: Scope,
    e: impl AsRef<str>,
    api: RwSignal<Option<crate::api::AuthorizedApi>>,
) {
    use leptos_router::*;

    if e.as_ref().contains("ExpiredSignature") {
        api.update(|a| *a = None);
        let navigate = use_navigate(cx);
        navigate(Page::Login.path(), Default::default()).expect("login page");
    }
}

pub fn goto(cx: Scope, page: impl AsRef<str>) -> Result<(), NavigationError> {
    let navigate = use_navigate(cx);
    navigate(page.as_ref(), Default::default())
}
