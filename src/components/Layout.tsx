import { useState, useEffect } from 'react';
import { Outlet, NavLink, useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { useChatStore, Conversation } from '../stores/chatStore';
import { getFriendlyModelName } from '../stores/modelStore';
import './Layout.css';

export default function Layout() {
    const [sidebarOpen, setSidebarOpen] = useState(true);
    const [searchQuery, setSearchQuery] = useState('');
    const navigate = useNavigate();
    const {
        conversations,
        setConversations,
        activeConversationId,
        setActiveConversation,
        setMessages,
    } = useChatStore();

    useEffect(() => {
        loadConversations();
    }, []);

    async function loadConversations() {
        try {
            const convos = await invoke<Conversation[]>('list_conversations');
            setConversations(convos);
        } catch (err) {
            console.error('Failed to load conversations:', err);
        }
    }

    async function handleNewChat() {
        setActiveConversation(null);
        setMessages([]);
        navigate('/chat');
    }

    async function handleSelectConversation(id: string) {
        setActiveConversation(id);
        try {
            const msgs = await invoke<any[]>('get_messages', { conversationId: id });
            setMessages(msgs);
        } catch (err) {
            console.error('Failed to load messages:', err);
        }
        navigate(`/chat/${id}`);
    }

    async function handleDeleteConversation(e: React.MouseEvent, id: string) {
        e.stopPropagation();
        try {
            await invoke('delete_conversation', { id });
            useChatStore.getState().removeConversation(id);
            if (activeConversationId === id) {
                setActiveConversation(null);
                setMessages([]);
                navigate('/chat');
            }
        } catch (err) {
            console.error('Failed to delete conversation:', err);
        }
    }

    const filteredConversations = conversations.filter((c) =>
        c.title.toLowerCase().includes(searchQuery.toLowerCase())
    );

    return (
        <div className="layout">
            <aside className={`sidebar ${sidebarOpen ? 'open' : 'collapsed'}`}>
                <div className="sidebar-header">
                    <button className="btn btn-primary btn-new-chat" onClick={handleNewChat}>
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <line x1="12" y1="5" x2="12" y2="19" />
                            <line x1="5" y1="12" x2="19" y2="12" />
                        </svg>
                        New Chat
                    </button>
                    <button
                        className="btn btn-icon btn-ghost sidebar-toggle"
                        onClick={() => setSidebarOpen(false)}
                        title="Collapse sidebar"
                    >
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <rect x="3" y="3" width="18" height="18" rx="2" />
                            <line x1="9" y1="3" x2="9" y2="21" />
                        </svg>
                    </button>
                </div>

                <div className="sidebar-search">
                    <input
                        className="input"
                        type="text"
                        placeholder="Search conversations..."
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                    />
                </div>

                <div className="sidebar-conversations">
                    {filteredConversations.length === 0 ? (
                        <p className="sidebar-empty">No conversations yet</p>
                    ) : (
                        filteredConversations.map((c) => (
                            <div
                                key={c.id}
                                className={`sidebar-conversation ${activeConversationId === c.id ? 'active' : ''}`}
                                onClick={() => handleSelectConversation(c.id)}
                            >
                                <div className="conversation-info">
                                    <span className="conversation-title">{c.title}</span>
                                    <span className="conversation-model">
                                        {getFriendlyModelName(c.model)}
                                    </span>
                                </div>
                                <button
                                    className="btn btn-icon btn-ghost conversation-delete"
                                    onClick={(e) => handleDeleteConversation(e, c.id)}
                                    title="Delete"
                                >
                                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                        <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m3 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6h14" />
                                    </svg>
                                </button>
                            </div>
                        ))
                    )}
                </div>

                <nav className="sidebar-nav">
                    <NavLink to="/chat" className={({ isActive }) => `nav-link ${isActive ? 'active' : ''}`}>
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z" />
                        </svg>
                        <span>Chat</span>
                    </NavLink>
                    <NavLink to="/models" className={({ isActive }) => `nav-link ${isActive ? 'active' : ''}`}>
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z" />
                        </svg>
                        <span>Models</span>
                    </NavLink>
                    <NavLink to="/settings" className={({ isActive }) => `nav-link ${isActive ? 'active' : ''}`}>
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <circle cx="12" cy="12" r="3" />
                            <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" />
                        </svg>
                        <span>Settings</span>
                    </NavLink>
                    <NavLink to="/plugins" className={({ isActive }) => `nav-link ${isActive ? 'active' : ''}`}>
                        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <rect x="2" y="2" width="8" height="8" rx="1" />
                            <rect x="14" y="2" width="8" height="8" rx="1" />
                            <rect x="2" y="14" width="8" height="8" rx="1" />
                            <rect x="14" y="14" width="8" height="8" rx="1" />
                        </svg>
                        <span>Plugins</span>
                    </NavLink>
                </nav>
            </aside>

            <main className="main-content">
                {!sidebarOpen && (
                    <button
                        className="btn btn-icon btn-ghost sidebar-expand-btn"
                        onClick={() => setSidebarOpen(true)}
                        title="Expand sidebar"
                    >
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <line x1="3" y1="6" x2="21" y2="6" />
                            <line x1="3" y1="12" x2="21" y2="12" />
                            <line x1="3" y1="18" x2="21" y2="18" />
                        </svg>
                    </button>
                )}
                <Outlet />
            </main>
        </div>
    );
}
