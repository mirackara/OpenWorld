import { useState, useEffect, useRef } from 'react';
import { useParams } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import ReactMarkdown from 'react-markdown';
import { useChatStore, type Message } from '../stores/chatStore';
import { useSettingsStore } from '../stores/settingsStore';
import { getFriendlyModelName } from '../stores/modelStore';
import './ChatPage.css';

export default function ChatPage() {
    const { conversationId } = useParams<{ conversationId?: string }>();
    const [inputValue, setInputValue] = useState('');
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const textareaRef = useRef<HTMLTextAreaElement>(null);

    const {
        activeConversationId,
        setActiveConversation,
        messages,
        setMessages,
        addMessage,
        isStreaming,
        setIsStreaming,
        streamingContent,
        appendStreamingContent,
        clearStreamingContent,
    } = useChatStore();
    const { defaultModel } = useSettingsStore();


    // Load conversation if URL has an ID
    useEffect(() => {
        if (conversationId && conversationId !== activeConversationId) {
            setActiveConversation(conversationId);
            loadMessages(conversationId);
        }
    }, [conversationId]);

    // Scroll to bottom on new messages
    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages, streamingContent]);

    // Listen for streaming tokens
    useEffect(() => {
        let unlisten: (() => void) | undefined;
        listen<{ conversation_id: string; content: string; done: boolean }>(
            'chat-stream-token',
            (event) => {
                const { content, done } = event.payload;
                if (content) {
                    appendStreamingContent(content);
                }
                if (done) {
                    setIsStreaming(false);
                }
            }
        ).then((fn) => {
            unlisten = fn;
        });
        return () => unlisten?.();
    }, []);



    async function loadMessages(convId: string) {
        try {
            const msgs = await invoke<Message[]>('get_messages', { conversationId: convId });
            setMessages(msgs);
        } catch (err) {
            console.error('Failed to load messages:', err);
        }
    }

    async function handleSend() {
        const content = inputValue.trim();
        if (!content || isStreaming) return;

        let convId = activeConversationId;

        // Create new conversation if needed
        if (!convId) {
            try {
                const title = content.slice(0, 50) + (content.length > 50 ? '...' : '');
                const conv = await invoke<any>('create_conversation', {
                    title,
                    model: defaultModel,
                });
                convId = conv.id;
                setActiveConversation(conv.id);

                // Refresh conversation list
                const convos = await invoke<any[]>('list_conversations');
                useChatStore.getState().setConversations(convos);
            } catch (err) {
                console.error('Failed to create conversation:', err);
                return;
            }
        }

        // Add user message
        const userMsg: Message = {
            id: crypto.randomUUID(),
            conversation_id: convId!,
            role: 'user',
            content,
            timestamp: new Date().toISOString(),
        };
        addMessage(userMsg);
        setInputValue('');
        setIsStreaming(true);
        clearStreamingContent();

        // Save user message
        try {
            await invoke('add_message', {
                conversationId: convId,
                role: 'user',
                content,
            });
        } catch (err) {
            console.error('Failed to save message:', err);
        }

        // Build message history for context
        const history = [...messages, userMsg].map((m) => ({
            role: m.role,
            content: m.content,
        }));

        // Send to Ollama via Tauri
        try {
            const fullResponse = await invoke<string>('send_message', {
                conversationId: convId,
                messages: history,
                model: defaultModel,
            });

            // IMPORTANT: Clear streaming state BEFORE adding the final message
            // to prevent both the streaming bubble and message bubble showing simultaneously
            clearStreamingContent();
            setIsStreaming(false);

            // Now add assistant message to local state
            const assistantMsg: Message = {
                id: crypto.randomUUID(),
                conversation_id: convId!,
                role: 'assistant',
                content: fullResponse,
                timestamp: new Date().toISOString(),
            };
            addMessage(assistantMsg);
        } catch (err) {
            console.error('Failed to send message:', err);
            clearStreamingContent();
            setIsStreaming(false);
        }
    }

    function handleKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSend();
        }
    }

    // Auto-resize textarea
    function handleInputChange(e: React.ChangeEvent<HTMLTextAreaElement>) {
        setInputValue(e.target.value);
        if (textareaRef.current) {
            textareaRef.current.style.height = 'auto';
            textareaRef.current.style.height = Math.min(textareaRef.current.scrollHeight, 200) + 'px';
        }
    }

    const noMessages = messages.length === 0 && !streamingContent;

    return (
        <div className="chat-page">
            {noMessages ? (
                <div className="chat-empty fade-in">
                    <div className="empty-logo">
                        <div className="logo-orb small">
                            <span>🌍</span>
                        </div>
                    </div>
                    <h2>What can I help you with?</h2>
                    <p>Start a conversation with your local AI</p>
                    <div className="empty-suggestions">
                        {[
                            'Explain quantum computing simply',
                            'Write a short story about space',
                            'Help me plan a weekend trip',
                            'Debug my Python code',
                        ].map((prompt) => (
                            <button
                                key={prompt}
                                className="btn btn-secondary suggestion-btn"
                                onClick={() => {
                                    setInputValue(prompt);
                                    textareaRef.current?.focus();
                                }}
                            >
                                {prompt}
                            </button>
                        ))}
                    </div>
                </div>
            ) : (
                <div className="chat-messages">
                    {messages.map((msg) => (
                        <div key={msg.id} className={`message message-${msg.role} slide-up`}>
                            <div className="message-avatar">
                                {msg.role === 'user' ? '👤' : '🌍'}
                            </div>
                            <div className="message-content">
                                <ReactMarkdown>{msg.content}</ReactMarkdown>
                            </div>
                        </div>
                    ))}
                    {isStreaming && streamingContent && (
                        <div className="message message-assistant slide-up">
                            <div className="message-avatar">🌍</div>
                            <div className="message-content">
                                <ReactMarkdown>{streamingContent}</ReactMarkdown>
                                <span className="streaming-cursor" />
                            </div>
                        </div>
                    )}
                    {isStreaming && !streamingContent && (
                        <div className="message message-assistant slide-up">
                            <div className="message-avatar">🌍</div>
                            <div className="message-content">
                                <div className="typing-indicator">
                                    <span /><span /><span />
                                </div>
                            </div>
                        </div>
                    )}
                    <div ref={messagesEndRef} />
                </div>
            )}

            <div className="chat-input-area">
                <div className="chat-input-container">
                    <textarea
                        ref={textareaRef}
                        className="chat-textarea"
                        placeholder="Message OpenWorld..."
                        value={inputValue}
                        onChange={handleInputChange}
                        onKeyDown={handleKeyDown}
                        rows={1}
                        disabled={isStreaming}
                    />
                    <button
                        className="btn btn-primary btn-icon send-btn"
                        onClick={handleSend}
                        disabled={!inputValue.trim() || isStreaming}
                    >
                        {isStreaming ? (
                            <div className="send-spinner" />
                        ) : (
                            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                <line x1="22" y1="2" x2="11" y2="13" />
                                <polygon points="22 2 15 22 11 13 2 9 22 2" />
                            </svg>
                        )}
                    </button>
                </div>
                <p className="chat-disclaimer">
                    {getFriendlyModelName(defaultModel)} · Running locally on your machine
                </p>
            </div>
        </div>
    );
}
