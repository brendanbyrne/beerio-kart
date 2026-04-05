import { useLocation, useNavigate } from 'react-router-dom'

export default function BottomNav() {
  const location = useLocation()
  const navigate = useNavigate()

  // Detect if we're on a session page
  const sessionMatch = location.pathname.match(/^\/session\/(.+)$/)
  const inSession = !!sessionMatch

  const tabs = [
    { path: '/', label: 'Home', icon: '\uD83C\uDFE0', disabled: false },
    {
      path: sessionMatch ? location.pathname : '/session',
      label: 'Session',
      icon: '\uD83C\uDFAE',
      disabled: !inSession,
    },
    { path: '/profile', label: 'Profile', icon: '\uD83D\uDC64', disabled: false },
  ]

  return (
    <nav className="fixed bottom-0 left-0 right-0 bg-white border-t border-gray-200 safe-area-pb">
      <div className="flex max-w-lg mx-auto">
        {tabs.map((tab) => {
          const isActive =
            inSession && tab.label === 'Session'
              ? true
              : !inSession && location.pathname === tab.path

          return (
            <button
              key={tab.label}
              onClick={() => !tab.disabled && navigate(tab.path)}
              disabled={tab.disabled}
              className={`flex-1 flex flex-col items-center py-2 min-h-[52px] transition-colors ${
                isActive
                  ? 'text-blue-500'
                  : tab.disabled
                    ? 'text-gray-300 cursor-not-allowed'
                    : 'text-gray-400 hover:text-gray-600'
              }`}
            >
              <span className="text-xl leading-none">{tab.icon}</span>
              <span className="text-[10px] font-medium mt-0.5">{tab.label}</span>
            </button>
          )
        })}
      </div>
    </nav>
  )
}
