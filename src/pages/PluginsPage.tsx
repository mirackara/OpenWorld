import './PluginsPage.css';

export default function PluginsPage() {
    return (
        <div className="plugins-page fade-in">
            <div className="plugins-center">
                <div className="plugins-icon">
                    <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="1.5">
                        <rect x="2" y="2" width="8" height="8" rx="2" />
                        <rect x="14" y="2" width="8" height="8" rx="2" />
                        <rect x="2" y="14" width="8" height="8" rx="2" />
                        <rect x="14" y="14" width="8" height="8" rx="2" opacity="0.3" strokeDasharray="3 3" />
                    </svg>
                </div>
                <h1>Plugins</h1>
                <p className="plugins-subtitle">
                    Extend OpenWorld with custom capabilities
                </p>
                <div className="plugins-preview card">
                    <h3>Coming Soon</h3>
                    <p>
                        Plugins will let you connect OpenWorld to your favorite tools and services.
                        Think web browsing, code execution, file management, and more — all running
                        locally on your machine.
                    </p>
                    <div className="plugins-examples">
                        <div className="plugin-example">
                            <span>🌐</span>
                            <span>Web Search</span>
                        </div>
                        <div className="plugin-example">
                            <span>💻</span>
                            <span>Code Runner</span>
                        </div>
                        <div className="plugin-example">
                            <span>📁</span>
                            <span>File Manager</span>
                        </div>
                        <div className="plugin-example">
                            <span>📧</span>
                            <span>Email</span>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    );
}
