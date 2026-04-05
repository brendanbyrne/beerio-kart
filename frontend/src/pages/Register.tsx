import { useState, type FormEvent } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { useAuth } from '../hooks/useAuth'

export default function Register() {
  const { register } = useAuth()
  const navigate = useNavigate()
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)

  async function handleSubmit(e: FormEvent) {
    e.preventDefault()
    setError(null)
    setSubmitting(true)

    const err = await register(username, password)
    setSubmitting(false)

    if (err) {
      setError(err)
    } else {
      // After registration, redirect to onboarding to set up preferences
      navigate('/onboarding')
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50 px-4">
      <form
        onSubmit={handleSubmit}
        className="w-full max-w-sm space-y-4 bg-white p-6 rounded-xl border border-gray-200 shadow-sm"
      >
        <h1 className="text-2xl font-bold text-gray-900 text-center">Register</h1>

        {error && <p className="text-red-500 text-sm text-center">{error}</p>}

        <div>
          <label htmlFor="username" className="block text-sm text-gray-600 mb-1">
            Username
          </label>
          <input
            id="username"
            type="text"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            className="w-full px-3 py-2.5 bg-gray-50 text-gray-900 rounded-lg border border-gray-200 focus:outline-none focus:border-blue-400"
            autoComplete="username"
            required
          />
        </div>

        <div>
          <label htmlFor="password" className="block text-sm text-gray-600 mb-1">
            Password
          </label>
          <input
            id="password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            className="w-full px-3 py-2.5 bg-gray-50 text-gray-900 rounded-lg border border-gray-200 focus:outline-none focus:border-blue-400"
            autoComplete="new-password"
            minLength={8}
            required
          />
          <p className="text-xs text-gray-400 mt-1">Must be at least 8 characters</p>
        </div>

        <button
          type="submit"
          disabled={submitting}
          className="w-full py-2.5 bg-blue-500 text-white font-semibold rounded-xl hover:bg-blue-600 disabled:bg-gray-300 transition-colors"
        >
          {submitting ? 'Creating account...' : 'Register'}
        </button>

        <p className="text-sm text-gray-400 text-center">
          Already have an account?{' '}
          <Link to="/login" className="text-blue-500 hover:underline font-medium">
            Log In
          </Link>
        </p>
      </form>
    </div>
  )
}
