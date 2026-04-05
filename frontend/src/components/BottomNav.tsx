import { useLocation, useNavigate } from 'react-router-dom'

const TABS = [
  { path: '/', label: 'Home', icon: '\uD83C\uDFE0' },
  { path: '/session', label: 'Session', icon: '\uD83C\uDFAE', disabled: true },
  { path: '/profile', label: 'Profile', icon: '\uD83D\uDC64' },
] as const

export default function BottomNav() {
  const location = useLocation()
  const navigate = useNavigate()

  return (
    <nav className="fixed bottom-0 left-0 right-0 bg-white border-t border-gray-200 safe-area-pb">
      <div className="flex max-w-lg mx-auto">
        {TABS.map((tab) => {
          const isActive = location.pathname === tab.path
          const isDisabled = 'disabled' in tab && tab.disabled

          return (
            <button
              key={tab.path}
              onClick={() => !isDisabled && navigate(tab.path)}
              disabled={isDisabled}
              className={`flex-1 flex flex-col items-center py-2 min-h-[52px] transition-colors ${
                isActive
                  ? 'text-blue-500'
                  : isDisabled
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
