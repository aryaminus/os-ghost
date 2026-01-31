import PropTypes from "prop-types";

const SettingsSidebar = ({ sections, activeSection, onSelect }) => {
  return (
    <aside className="settings-sidebar" aria-label="Settings navigation">
      <nav className="settings-nav">
        {sections.map((section) => (
          <button
            key={section.id}
            type="button"
            className={`settings-nav-item ${
              activeSection === section.id ? "active" : ""
            }`}
            onClick={() => onSelect(section.id)}
          >
            <span className="settings-nav-label">{section.label}</span>
          </button>
        ))}
      </nav>
    </aside>
  );
};

SettingsSidebar.propTypes = {
  sections: PropTypes.arrayOf(
    PropTypes.shape({
      id: PropTypes.string.isRequired,
      label: PropTypes.string.isRequired,
    })
  ).isRequired,
  activeSection: PropTypes.string.isRequired,
  onSelect: PropTypes.func.isRequired,
};

export default SettingsSidebar;
