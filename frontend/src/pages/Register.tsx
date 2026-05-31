import { useActionState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import * as z from 'zod';
import { useAuth } from '../hooks/useAuth';
import { SubmitButton } from '../components/SubmitButton';

type RegisterState = { error: string | null };

const INITIAL: RegisterState = { error: null };

// Mirror of the backend's username/password rules. The 8-char minimum is
// also enforced by the input's `minLength`; this schema is the
// submit-time backstop per react.md § 8.
const RegisterFormSchema = z.object({
  username: z.string().min(1),
  password: z.string().min(8),
});

export function Register() {
  const { register } = useAuth();
  const navigate = useNavigate();

  const [state, submit] = useActionState<RegisterState, FormData>(
    async (_prev, formData) => {
      const parsed = RegisterFormSchema.safeParse(Object.fromEntries(formData));
      if (!parsed.success) {
        return { error: 'Password must be at least 8 characters' };
      }
      const err = await register(parsed.data.username, parsed.data.password);
      if (err) return { error: err };
      navigate('/onboarding');
      return { error: null };
    },
    INITIAL,
  );

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50 px-4">
      <title>Register · Beerio Kart</title>
      <form
        action={submit}
        className="w-full max-w-sm space-y-4 bg-white p-6 rounded-xl border border-gray-200 shadow-sm"
      >
        <h1 className="text-2xl font-bold text-gray-900 text-center">
          Register
        </h1>

        {state.error && (
          <p className="text-danger text-sm text-center">{state.error}</p>
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
          className="w-full py-2.5 bg-brand-primary text-white font-semibold rounded-xl hover:bg-brand-primary-hover transition-colors"
          pendingLabel="Creating account..."
        >
          Register
        </SubmitButton>

        <p className="text-sm text-gray-400 text-center">
          Already have an account?{' '}
          <Link
            to="/login"
            className="text-brand-primary hover:underline font-medium"
          >
            Log In
          </Link>
        </p>
      </form>
    </div>
  );
}
