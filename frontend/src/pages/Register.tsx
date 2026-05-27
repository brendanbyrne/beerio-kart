import { useActionState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';
import { SubmitButton } from '../components/SubmitButton';

type RegisterState = { error: string | null };

const INITIAL: RegisterState = { error: null };

export function Register() {
  const { register } = useAuth();
  const navigate = useNavigate();

  const [state, submit] = useActionState<RegisterState, FormData>(
    async (_prev, formData) => {
      const username = formData.get('username');
      const password = formData.get('password');
      if (typeof username !== 'string' || typeof password !== 'string') {
        return { error: 'Invalid form data' };
      }
      const err = await register(username, password);
      if (err) return { error: err };
      navigate('/onboarding');
      return { error: null };
    },
    INITIAL,
  );

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50 px-4">
      <form
        action={submit}
        className="w-full max-w-sm space-y-4 bg-white p-6 rounded-xl border border-gray-200 shadow-sm"
      >
        <h1 className="text-2xl font-bold text-gray-900 text-center">
          Register
        </h1>

        {state.error && (
          <p className="text-red-500 text-sm text-center">{state.error}</p>
        )}

        <div>
          <label
            htmlFor="username"
            className="block text-sm text-gray-600 mb-1"
          >
            Username
          </label>
          <input
            id="username"
            name="username"
            type="text"
            className="w-full px-3 py-2.5 bg-gray-50 text-gray-900 rounded-lg border border-gray-200 focus:outline-none focus:border-blue-400"
            autoComplete="username"
            required
          />
        </div>

        <div>
          <label
            htmlFor="password"
            className="block text-sm text-gray-600 mb-1"
          >
            Password
          </label>
          <input
            id="password"
            name="password"
            type="password"
            className="w-full px-3 py-2.5 bg-gray-50 text-gray-900 rounded-lg border border-gray-200 focus:outline-none focus:border-blue-400"
            autoComplete="new-password"
            minLength={8}
            required
          />
          <p className="text-xs text-gray-400 mt-1">
            Must be at least 8 characters
          </p>
        </div>

        <SubmitButton
          className="w-full py-2.5 bg-blue-500 text-white font-semibold rounded-xl hover:bg-blue-600 transition-colors"
          pendingLabel="Creating account..."
        >
          Register
        </SubmitButton>

        <p className="text-sm text-gray-400 text-center">
          Already have an account?{' '}
          <Link
            to="/login"
            className="text-blue-500 hover:underline font-medium"
          >
            Log In
          </Link>
        </p>
      </form>
    </div>
  );
}
