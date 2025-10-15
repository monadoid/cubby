export function Form() {
	return (
		<div class="max-w-md mx-auto">
			<form 
				action="/posts" 
				method="post" 
				hx-trigger="reset" 
				hx-vals={{ foo: "bar" }}
				class="space-y-4"
			>
				<div>
					<label for="author" class="block text-sm font-medium mb-2">
						Author
					</label>
					<input 
						type="text" 
						name="author" 
						id="author"
						class="w-full px-3 py-2 border border-gray-600 bg-black text-white rounded focus:outline-none focus:border-white"
						placeholder="enter your name"
					/>
				</div>
				<div>
					<label for="message" class="block text-sm font-medium mb-2">
						Message
					</label>
					<textarea 
						name="message" 
						id="message"
						rows={4}
						class="w-full px-3 py-2 border border-gray-600 bg-black text-white rounded focus:outline-none focus:border-white"
						placeholder="what's on your mind?"
					></textarea>
				</div>
				<button 
					type="submit"
					class="w-full px-4 py-2 bg-white text-black font-bold rounded hover:bg-gray-200 transition-colors"
				>
					Submit
				</button>
			</form>
		</div>
	);
}
