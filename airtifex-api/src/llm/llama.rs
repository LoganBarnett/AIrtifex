use crate::config::LlmConfig;
use crate::id::Uuid;
use crate::models::chat_entry::ChatEntry;
use airtifex_core::llm::{ChatEntryType, ChatStreamResult};

use llama_rs::{
    InferenceError, InferenceParameters, InferenceSession, InferenceSessionParameters,
    LoadProgress, Model, ModelKVMemoryType, TokenBias, Vocabulary,
};
use rand::{rngs::ThreadRng, thread_rng};
use std::{collections::VecDeque, sync::Arc};
use tokio::runtime::Runtime;

use flume::{unbounded, Receiver, Sender};

const ANSWER_PREFIX: &str = "Llama: ";
const USER_PREFIX: &str = "User: ";
const CONVERSATION_PROMPT: &str = r#"Your name is Llama and you are a helpful virtual assistant.
As Llama, you fulfill users request in the most effective way and your answer is never empty.
Below is a dialog between a user and you.
Write a response to the request in the '### Request:' section that appropriately completes the request.

### Conversation:
{{HISTORY}}

### Request:
{{PROMPT}}

### Response:"#;

#[derive(Debug)]
pub struct ChatData {
    pub user: String,
    pub conversation_id: Uuid,
    pub history: Vec<ChatEntry>,
}

#[derive(Debug)]
pub struct InferenceRequest {
    /// The channel to send the tokens to.
    pub tx_tokens: Sender<ChatStreamResult>,

    pub chat_data: Option<ChatData>,
    pub num_predict: Option<usize>,
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub n_batch: Option<usize>,
    pub top_k: Option<usize>,
    pub top_p: Option<f32>,
    pub repeat_penalty: Option<f32>,
    pub temp: Option<f32>,
    pub play_back_tokens: bool,
}

#[derive(Debug)]
pub struct SaveDataRequest {
    pub conversation_id: Uuid,
    pub input: String,
    pub output: String,
}

#[derive(Default)]
struct InferenceState {
    pub processed_tokens: usize,
    pub answer: String,
    pub processed_prompt: String,
    pub is_finished: bool,
}

pub fn initialize_model_and_handle_inferences(
    db: Arc<crate::DbPool>,
    config: LlmConfig,
    runtime: Arc<Runtime>,
) -> Sender<InferenceRequest> {
    static REQUEST_QUEUE: std::sync::RwLock<VecDeque<InferenceRequest>> =
        std::sync::RwLock::new(VecDeque::new());

    // Create a channel and thread responsible for saving chat entries to database
    let (tx_results, rx_results): (Sender<SaveDataRequest>, Receiver<SaveDataRequest>) =
        unbounded();
    std::thread::spawn(move || loop {
        if let Ok(save_data_request) = rx_results.recv() {
            let user = ChatEntry::new_user(
                save_data_request.conversation_id.clone(),
                save_data_request.input,
            );
            let bot = ChatEntry::new_bot(
                save_data_request.conversation_id.clone(),
                save_data_request.output,
            );
            let db = db.clone();
            let _ = runtime.spawn(async move {
                if let Err(e) = user.create(&db).await {
                    log::error!("failed to save user chat entry - {e}")
                }
                if let Err(e) = bot.create(&db).await {
                    log::error!("failed to save bot chat entry - {e}")
                }
            });
        } else {
            log::error!("all channels closed");
            break;
        }
    });

    // Create a thread that'll receive InferenceRequests
    let (tx_request, rx_request) = unbounded();
    std::thread::spawn(move || {
        let rx: Receiver<InferenceRequest> = rx_request.clone();
        let mut temp_queue = VecDeque::new();
        loop {
            if let Ok(inference_request) = rx.recv() {
                if let Ok(mut queue) = REQUEST_QUEUE.try_write() {
                    while let Some(req) = temp_queue.pop_front() {
                        queue.push_back(req);
                    }
                    queue.push_back(inference_request);
                } else {
                    temp_queue.push_back(inference_request);
                }
            }
        }
    });

    // Create a thread that will handle inference
    std::thread::spawn(move || {
        let mut inference_session_manager = InferenceSessionManager::new(config);
        let mut running_sessions = VecDeque::new();
        let mut rng = thread_rng();

        loop {
            let mut free_spots =
                inference_session_manager.config.max_inference_sessions - running_sessions.len();
            if free_spots > 0 {
                if let Ok(mut queue) = REQUEST_QUEUE.try_write() {
                    while let Some(inference_request) = queue.pop_front() && free_spots > 0 {
                        let mut session = inference_session_manager.get_inference_session(inference_request);

                        if let Err(e) = session.feed_prompt(&inference_session_manager.model, &inference_session_manager.vocabulary) {
                            log::error!("failed to initialize inference session - {e}");
                        } else {
                            running_sessions.push_back(session);
                            free_spots -= 1;
                        }
                    }
                }
            }
            for session in &mut running_sessions {
                if session.state.processed_tokens
                    <= session.request.num_predict.unwrap_or(usize::MAX)
                {
                    if let Err(e) =
                        session.infer_next_token(&inference_session_manager, &mut rng, &tx_results)
                    {
                        log::error!("{e}");
                    }
                } else {
                    log::debug!("already infered max number of tokens for session");
                    session.state.is_finished = true;
                }
            }

            running_sessions.retain(|s| !s.state.is_finished);

            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    });

    tx_request
}

struct InferenceSessionManager {
    model: llama_rs::Model,
    vocabulary: llama_rs::Vocabulary,
    config: LlmConfig,
}

impl InferenceSessionManager {
    fn new(config: LlmConfig) -> Self {
        // Load model
        let (model, vocabulary) =
            llama_rs::Model::load(&config.model_path, config.num_ctx_tokens, |progress| {
                match progress {
                    LoadProgress::HyperparametersLoaded(hparams) => {
                        log::debug!("Loaded hyperparameters {hparams:#?}")
                    }
                    //LoadProgress::BadToken { index } => {
                    //log::info!("Warning: Bad token in vocab at index {index}")
                    //}
                    LoadProgress::ContextSize { bytes } => log::info!(
                        "ggml ctx size = {:.2} MB\n",
                        bytes as f64 / (1024.0 * 1024.0)
                    ),
                    LoadProgress::PartLoading {
                        file,
                        current_part,
                        total_parts,
                    } => {
                        let current_part = current_part + 1;
                        log::info!(
                            "Loading model part {}/{} from '{}'\n",
                            current_part,
                            total_parts,
                            file.to_string_lossy(),
                        )
                    }
                    LoadProgress::PartTensorLoaded {
                        current_tensor,
                        tensor_count,
                        ..
                    } => {
                        let current_tensor = current_tensor + 1;
                        if current_tensor % 8 == 0 {
                            log::info!("Loaded tensor {current_tensor}/{tensor_count}");
                        }
                    }
                    LoadProgress::PartLoaded {
                        file,
                        byte_size,
                        tensor_count,
                    } => {
                        log::info!("Loading of '{}' complete", file.to_string_lossy());
                        log::info!(
                            "Model size = {:.2} MB / num tensors = {}",
                            byte_size as f64 / 1024.0 / 1024.0,
                            tensor_count
                        );
                    }
                }
            })
            .expect("Could not load model");

        Self {
            model,
            vocabulary,
            config,
        }
    }

    fn get_inference_session(&mut self, request: InferenceRequest) -> RunningInferenceSession {
        let inference_session_params = {
            let mem_typ = if self.config.float16 {
                ModelKVMemoryType::Float16
            } else {
                ModelKVMemoryType::Float32
            };
            InferenceSessionParameters {
                memory_k_type: mem_typ,
                memory_v_type: mem_typ,
                repetition_penalty_last_n: self.config.repeat_last_n,
            }
        };

        let params = InferenceParameters {
            n_threads: self.config.num_threads,
            n_batch: request.n_batch.unwrap_or(self.config.batch_size),
            top_k: request.top_k.unwrap_or(self.config.top_k),
            top_p: request.top_p.unwrap_or(self.config.top_p),
            repeat_penalty: request.repeat_penalty.unwrap_or(self.config.repeat_penalty),
            temperature: request.temp.unwrap_or(self.config.temperature),
            bias_tokens: TokenBias::default(),
            play_back_previous_tokens: request.play_back_tokens,
        };

        let prompt = if let Some(chat) = &request.chat_data {
            let history = chat.history.iter().fold(String::new(), |mut acc, x| {
                let prefix = match x.entry_type {
                    ChatEntryType::Bot => ANSWER_PREFIX,
                    ChatEntryType::User => USER_PREFIX,
                };
                acc.push_str(prefix);
                acc.push_str(&x.content);
                acc.push('\n');
                acc
            });
            let user_prompt = format!("{USER_PREFIX}{}", request.prompt);
            request
                .system_prompt
                .as_deref()
                .unwrap_or(CONVERSATION_PROMPT)
                .replace("{{HISTORY}}", &history)
                .replace("{{PROMPT}}", &user_prompt)
        } else {
            request.prompt.clone()
        };

        RunningInferenceSession {
            id: Uuid::new_v4(),
            session: self.model.start_session(inference_session_params),
            params,
            request,
            state: InferenceState {
                processed_prompt: prompt,
                ..Default::default()
            },
        }
    }
}

struct RunningInferenceSession {
    pub id: Uuid,
    pub session: InferenceSession,
    pub params: InferenceParameters,
    pub request: InferenceRequest,
    pub state: InferenceState,
}

impl RunningInferenceSession {
    fn feed_prompt(&mut self, model: &Model, vocab: &Vocabulary) -> Result<(), crate::Error> {
        log::debug!(
            "[{}] Feeding prompt `{}`",
            self.id,
            self.state.processed_prompt
        );
        self.session
            .feed_prompt(
                model,
                vocab,
                &self.params,
                &self.state.processed_prompt,
                move |b| {
                    log::trace!("[prompt] {}", String::from_utf8_lossy(b));
                    Ok::<(), InferenceError>(())
                },
            )
            .map_err(crate::Error::from)
    }

    fn save_results(&mut self, tx_results: &Sender<SaveDataRequest>) {
        self.state.is_finished = true;
        if let Some(chat) = &self.request.chat_data {
            log::debug!("saving chat data");
            let output = self.state.answer.clone();
            if !output.is_empty() {
                if let Err(e) = tx_results.try_send(SaveDataRequest {
                    conversation_id: chat.conversation_id,
                    input: self.request.prompt.clone(),
                    output,
                }) {
                    log::error!("failed to save chat entries - {e}");
                }
            }
        }
    }

    fn infer_next_token(
        &mut self,
        inference_session_manager: &InferenceSessionManager,
        rng: &mut ThreadRng,
        tx_results: &Sender<SaveDataRequest>,
    ) -> Result<(), crate::Error> {
        log::debug!("inferering next valid utf-8 token");
        let mut buf = llama_rs::TokenUtf8Buffer::new();

        loop {
            let token = match self.session.infer_next_token(
                &inference_session_manager.model,
                &inference_session_manager.vocabulary,
                &self.params,
                rng,
            ) {
                Ok(token) => token,
                Err(InferenceError::EndOfText) => {
                    log::debug!("end of inference");
                    self.save_results(tx_results);
                    break;
                }
                Err(e) => return Err(e.into()),
            };

            if let Some(valid_token) = buf.push(token) {
                self.state.answer.push_str(&valid_token);
                self.state.processed_tokens += 1;
                log::debug!("Sending token {} to receiver.", valid_token);
                match self.request.tx_tokens.send(Ok(valid_token)) {
                    Ok(_) => {
                        break;
                    }
                    Err(e) => {
                        // The receiver has been dropped.
                        self.save_results(tx_results);
                        return Err(crate::Error::InferenceSend(e));
                    }
                }
            }
        }

        Ok(())
    }
}
