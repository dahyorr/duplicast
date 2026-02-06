import { useStats } from '../hooks';

export default function Dashboard() {
  const { data: stats, isLoading, error } = useStats();

  if (isLoading) {
    return <div className="loading">Loading stats...</div>;
  }

  if (error) {
    return <div className="error">Error loading stats: {error.message}</div>;
  }

  return (
    <div className="dashboard">
      <h1>Duplicast Dashboard</h1>

      <div className="stats-grid">
        <div className="stat-card">
          <div className="stat-value">{stats?.active_streams || 0}</div>
          <div className="stat-label">Active Streams</div>
        </div>

        <div className="stat-card">
          <div className="stat-value">{stats?.active_relays || 0}/{stats?.total_relays || 0}</div>
          <div className="stat-label">Active Relays</div>
        </div>

        <div className="stat-card">
          <div className="stat-value">
            {stats?.total_bitrate_mbps.toFixed(2) || 0} Mbps
          </div>
          <div className="stat-label">Total Bitrate</div>
        </div>

        <div className="stat-card">
          <div className="stat-value">
            {((stats?.total_bytes || 0) / 1024 / 1024).toFixed(2)} MB
          </div>
          <div className="stat-label">Total Data</div>
        </div>
      </div>
    </div>
  );
}
